//! Windows listener: Raw Input with `RIDEV_INPUTSINK` on a hidden window owned by a thread of
//! its own. Raw input is a copy of the input stream: unlike a hotkey it does not consume the key,
//! and unlike a low-level hook it does not sit in the system's input path (a slow hook would lag
//! every mouse move in the game).
//!
//! - Raw input registration is per process and device class; this replaces tao's own startup
//!   registration (device events), which Tauri does not use.
//! - Devices are registered only while a binding is set, the mouse only for a mouse trigger
//!   (a gaming mouse reports ~1000 moves per second).
//! - Windows does not deliver input to a normal app while an elevated (administrator) window has
//!   focus: a game run as administrator needs CallOut run as administrator too.
//! - While transmitting, a timer checks every 250 ms that the trigger is still physically down: a
//!   key-up lost to the secure desktop (Ctrl+Alt+Del, Win+L) must not leave the microphone open.

use crate::{Binding, Error, Input, Matcher, MouseButton, Trigger};
use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MapVirtualKeyW, MAPVK_VSC_TO_VK_EX, VK_MBUTTON, VK_XBUTTON1, VK_XBUTTON2,
};
use windows_sys::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
    RIDEV_INPUTSINK, RIDEV_REMOVE, RID_INPUT, RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, KillTimer,
    PostMessageW, PostQuitMessage, RegisterClassW, SetTimer, MSG, RI_KEY_BREAK, RI_KEY_E0,
    RI_MOUSE_BUTTON_4_DOWN, RI_MOUSE_BUTTON_4_UP, RI_MOUSE_BUTTON_5_DOWN, RI_MOUSE_BUTTON_5_UP,
    RI_MOUSE_MIDDLE_BUTTON_DOWN, RI_MOUSE_MIDDLE_BUTTON_UP, WM_APP, WM_CLOSE, WM_DESTROY, WM_INPUT,
    WM_TIMER, WNDCLASSW, WS_POPUP,
};

/// Posted by `set_binding`: read the shared binding and (un)register devices.
const WM_REBIND: u32 = WM_APP + 1;
const TIMER_ID: usize = 1;
const CHECK_MS: u32 = 250;
// HID usage page "generic desktop": mouse and keyboard.
const USAGE_PAGE: u16 = 0x01;
const USAGE_MOUSE: u16 = 0x02;
const USAGE_KEYBOARD: u16 = 0x06;

type Callback = Box<dyn Fn(bool) + Send>;

struct State {
    matcher: Matcher,
    shared: Arc<Mutex<Option<Binding>>>,
    on_change: Callback,
    keyboard: bool,
    mouse: bool,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Runs the listener thread; dropping it stops the thread (releasing a held push-to-talk).
pub struct Listener {
    /// The hidden window, as an integer: `HWND` is a raw pointer and not `Send`.
    hwnd: usize,
    shared: Arc<Mutex<Option<Binding>>>,
    thread: Option<JoinHandle<()>>,
}

impl Listener {
    /// Starts with no binding (nothing is received). `on_change(true/false)` is called on the
    /// listener thread when push-to-talk is pressed / released.
    pub fn start(on_change: impl Fn(bool) + Send + 'static) -> Result<Self, Error> {
        let shared = Arc::new(Mutex::new(None));
        let (tx, rx) = mpsc::channel();
        let state_shared = shared.clone();
        let thread = std::thread::Builder::new()
            .name("callout-ptt".into())
            .spawn(move || run(state_shared, Box::new(on_change), tx))
            .map_err(|e| Error::Os(e.to_string()))?;
        match rx.recv() {
            Ok(Ok(hwnd)) => Ok(Self {
                hwnd,
                shared,
                thread: Some(thread),
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => Err(Error::Os("listener thread ended".into())),
        }
    }

    /// Sets the binding, or clears it (`None`: stop receiving input). A transmission in progress
    /// is ended either way.
    pub fn set_binding(&self, binding: Option<Binding>) {
        *self.shared.lock().unwrap() = binding;
        // SAFETY: posting to a window handle has no memory effects; it fails if the window is gone.
        unsafe { PostMessageW(self.hwnd as HWND, WM_REBIND, 0, 0) };
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        // SAFETY: as in `set_binding`.
        unsafe { PostMessageW(self.hwnd as HWND, WM_CLOSE, 0, 0) };
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(
    shared: Arc<Mutex<Option<Binding>>>,
    on_change: Callback,
    ready: mpsc::Sender<Result<usize, Error>>,
) {
    let class: Vec<u16> = "CallOutPushToTalk\0".encode_utf16().collect();
    // SAFETY: window class and window setup for this thread; `class` outlives both calls. A
    // second registration of the class (a second listener) fails harmlessly.
    let hwnd = unsafe {
        let hinstance = GetModuleHandleW(std::ptr::null());
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance,
            lpszClassName: class.as_ptr(),
            ..Default::default()
        };
        RegisterClassW(&wc);
        // Hidden (no WS_VISIBLE) top-level window: input-sink registrations need a real window.
        CreateWindowExW(
            0,
            class.as_ptr(),
            std::ptr::null(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        let _ = ready.send(Err(Error::Os(std::io::Error::last_os_error().to_string())));
        return;
    }
    STATE.with(|s| {
        *s.borrow_mut() = Some(State {
            matcher: Matcher::new(),
            shared,
            on_change,
            keyboard: false,
            mouse: false,
        })
    });
    let _ = ready.send(Ok(hwnd as usize));
    let mut msg = MSG::default();
    // SAFETY: the standard message loop of this thread; `msg` lives across the calls.
    unsafe {
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            DispatchMessageW(&msg);
        }
    }
    STATE.with(|s| s.borrow_mut().take());
}

fn with_state(f: impl FnOnce(&mut State)) {
    STATE.with(|s| {
        if let Some(st) = s.borrow_mut().as_mut() {
            f(st)
        }
    });
}

/// Reports a change and keeps the "still held?" timer running only while transmitting.
fn changed(hwnd: HWND, st: &State, pressed: bool) {
    (st.on_change)(pressed);
    // SAFETY: timers on our own window.
    unsafe {
        if pressed {
            SetTimer(hwnd, TIMER_ID, CHECK_MS, None);
        } else {
            KillTimer(hwnd, TIMER_ID);
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_INPUT => {
            // SAFETY: for WM_INPUT, lparam is the raw input handle.
            let events = unsafe { read_input(lparam as HRAWINPUT) };
            with_state(|st| {
                for input in events.into_iter().flatten() {
                    if let Some(p) = st.matcher.feed(input) {
                        changed(hwnd, st, p);
                    }
                }
            });
            // WM_INPUT must reach DefWindowProc so the system can clean up the raw input buffer.
        }
        WM_REBIND => {
            with_state(|st| {
                let binding = *st.shared.lock().unwrap();
                if let Some(p) = st.matcher.set_binding(binding) {
                    changed(hwnd, st, p);
                }
                register(hwnd, st, binding);
            });
            return 0;
        }
        WM_TIMER if wparam == TIMER_ID => {
            with_state(|st| {
                let held = st
                    .matcher
                    .binding()
                    .is_some_and(|b| physically_down(b.trigger));
                if !held {
                    if let Some(p) = st.matcher.release() {
                        changed(hwnd, st, p);
                    }
                }
            });
            return 0;
        }
        WM_CLOSE => {
            // SAFETY: our own window, on its own thread.
            unsafe { DestroyWindow(hwnd) };
            return 0;
        }
        WM_DESTROY => {
            with_state(|st| {
                if let Some(p) = st.matcher.set_binding(None) {
                    changed(hwnd, st, p);
                }
                register(hwnd, st, None);
            });
            // SAFETY: ends this thread's message loop.
            unsafe { PostQuitMessage(0) };
            return 0;
        }
        _ => {}
    }
    // SAFETY: default handling of a message for our window.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// The key, or the mouse button transitions (one mouse report can carry several), of a raw
/// input event.
///
/// # Safety
/// `h` must be the handle of the WM_INPUT message being handled.
unsafe fn read_input(h: HRAWINPUT) -> [Option<Input>; 3] {
    let mut none = [None; 3];
    let mut raw = RAWINPUT::default();
    let mut size = size_of::<RAWINPUT>() as u32;
    // SAFETY: `raw` is a RAWINPUT-sized buffer and `size` says so; keyboard and mouse reports
    // fit (only HID reports can be larger, and those are not registered).
    let n = unsafe {
        GetRawInputData(
            h,
            RID_INPUT,
            (&mut raw as *mut RAWINPUT).cast::<c_void>(),
            &mut size,
            size_of::<RAWINPUTHEADER>() as u32,
        )
    };
    if n == 0 || n == u32::MAX {
        return none;
    }
    match raw.header.dwType {
        RIM_TYPEKEYBOARD => {
            // SAFETY: the header says this is a keyboard report.
            let k = unsafe { raw.data.keyboard };
            let flags = u32::from(k.Flags);
            none[0] = Some(Input::Key {
                scan: k.MakeCode,
                e0: flags & RI_KEY_E0 != 0,
                down: flags & RI_KEY_BREAK == 0,
            });
        }
        RIM_TYPEMOUSE => {
            // SAFETY: the header says this is a mouse report.
            let flags = u32::from(unsafe { raw.data.mouse.Anonymous.Anonymous.usButtonFlags });
            let buttons = [
                (
                    MouseButton::Middle,
                    RI_MOUSE_MIDDLE_BUTTON_DOWN,
                    RI_MOUSE_MIDDLE_BUTTON_UP,
                ),
                (
                    MouseButton::Back,
                    RI_MOUSE_BUTTON_4_DOWN,
                    RI_MOUSE_BUTTON_4_UP,
                ),
                (
                    MouseButton::Forward,
                    RI_MOUSE_BUTTON_5_DOWN,
                    RI_MOUSE_BUTTON_5_UP,
                ),
            ];
            for (slot, (button, down, up)) in none.iter_mut().zip(buttons) {
                if flags & (down | up) != 0 {
                    *slot = Some(Input::Mouse {
                        button,
                        down: flags & down != 0,
                    });
                }
            }
        }
        _ => {}
    }
    none
}

/// Registers the keyboard (any binding: modifiers are tracked from it) and the mouse (mouse
/// trigger only), or removes them.
fn register(hwnd: HWND, st: &mut State, binding: Option<Binding>) {
    let want_keyboard = binding.is_some();
    let want_mouse = matches!(
        binding,
        Some(Binding {
            trigger: Trigger::Mouse(_),
            ..
        })
    );
    for (usage, want, have) in [
        (USAGE_KEYBOARD, want_keyboard, &mut st.keyboard),
        (USAGE_MOUSE, want_mouse, &mut st.mouse),
    ] {
        if want == *have {
            continue;
        }
        let device = RAWINPUTDEVICE {
            usUsagePage: USAGE_PAGE,
            usUsage: usage,
            dwFlags: if want { RIDEV_INPUTSINK } else { RIDEV_REMOVE },
            hwndTarget: if want { hwnd } else { std::ptr::null_mut() },
        };
        // SAFETY: one valid RAWINPUTDEVICE with its size.
        let ok =
            unsafe { RegisterRawInputDevices(&device, 1, size_of::<RAWINPUTDEVICE>() as u32) } != 0;
        if ok {
            *have = want;
        } else {
            tracing::warn!(
                usage,
                want,
                "raw input registration failed: {}",
                std::io::Error::last_os_error()
            );
        }
    }
}

/// Whether the trigger is down right now (asynchronous key state). If its virtual key cannot be
/// determined, assume it is, rather than cutting the user off.
fn physically_down(trigger: Trigger) -> bool {
    let vk = match trigger {
        Trigger::Key { scan, e0 } => {
            let code = u32::from(scan) | if e0 { 0xE000 } else { 0 };
            // SAFETY: pure lookup.
            let vk = unsafe { MapVirtualKeyW(code, MAPVK_VSC_TO_VK_EX) };
            if vk == 0 {
                return true;
            }
            vk as i32
        }
        Trigger::Mouse(MouseButton::Middle) => i32::from(VK_MBUTTON),
        Trigger::Mouse(MouseButton::Back) => i32::from(VK_XBUTTON1),
        Trigger::Mouse(MouseButton::Forward) => i32::from(VK_XBUTTON2),
    };
    // SAFETY: pure query; the high bit is set while the key is down.
    let state = unsafe { GetAsyncKeyState(vk) };
    state < 0
}
