//! macOS listener: a listen-only Quartz event tap on a thread of its own. A listen-only tap gets a
//! copy of each event and cannot change or drop it, so the key still reaches the game.
//!
//! - macOS asks the user once ("Input Monitoring", System Settings → Privacy & Security) before
//!   any app may observe other apps' keystrokes. Until it is allowed no tap can be created; the
//!   listener asks for it the first time a binding is set and keeps retrying quietly, so it starts
//!   working as soon as the user allows it. [`has_input_access`] tells the UI to explain this.
//! - The tap exists only while a binding is set, and listens to the mouse only for a mouse
//!   trigger (a gaming mouse reports ~1000 moves per second; only button events are asked for).
//! - macOS turns a tap off when its callback is slow or on some user input; it is turned back on
//!   at once.
//! - While transmitting, every slice of the loop (100 ms) checks that the trigger is still
//!   physically down: a key-up the tap missed (secure input, a password field) must not leave the
//!   microphone open.

use crate::mac_keys::{modifier_flag, to_kvk, to_scan};
use crate::{Binding, Error, Input, Matcher, MouseButton, Trigger};
use objc2_core_foundation::{
    kCFRunLoopDefaultMode, CFMachPort, CFRetained, CFRunLoop, CFRunLoopSource,
};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventMask, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
    CGEventTapOptions, CGEventTapPlacement, CGEventTapProxy, CGEventType, CGMouseButton,
    CGPreflightListenEventAccess, CGRequestListenEventAccess,
};
use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

/// How long the run loop runs between checks of the command channel and the held trigger.
const SLICE: Duration = Duration::from_millis(100);

type Callback = Box<dyn Fn(bool) + Send>;

enum Cmd {
    Rebind(Option<Binding>),
    Stop,
}

struct Tap {
    port: CFRetained<CFMachPort>,
    source: CFRetained<CFRunLoopSource>,
    mouse: bool,
}

struct State {
    matcher: Matcher,
    on_change: Callback,
    tap: Option<Tap>,
    asked_access: bool,
    /// A failed tap creation is retried every slice but logged once.
    warned_tap: bool,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Whether macOS lets this app observe keystrokes in other apps (Input Monitoring).
pub fn has_input_access() -> bool {
    CGPreflightListenEventAccess()
}

/// Asks macOS for Input Monitoring: shows the system prompt the first time and lists CallOut in
/// System Settings → Privacy & Security → Input Monitoring (an app appears there only after it
/// asked). Returns whether access is already granted.
pub fn request_input_access() -> bool {
    CGRequestListenEventAccess()
}

/// Runs the listener thread; dropping it stops the thread (releasing a held push-to-talk).
pub struct Listener {
    tx: mpsc::Sender<Cmd>,
    thread: Option<JoinHandle<()>>,
}

impl Listener {
    /// Starts with no binding (nothing is received). `on_change(true/false)` is called on the
    /// listener thread when push-to-talk is pressed / released.
    pub fn start(on_change: impl Fn(bool) + Send + 'static) -> Result<Self, Error> {
        let (tx, rx) = mpsc::channel();
        let on_change: Callback = Box::new(on_change);
        let thread = std::thread::Builder::new()
            .name("callout-ptt".into())
            .spawn(move || run(rx, on_change))
            .map_err(|e| Error::Os(e.to_string()))?;
        Ok(Self {
            tx,
            thread: Some(thread),
        })
    }

    /// Sets the binding, or clears it (`None`: stop receiving input). A transmission in progress
    /// is ended either way.
    pub fn set_binding(&self, binding: Option<Binding>) {
        let _ = self.tx.send(Cmd::Rebind(binding));
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(rx: mpsc::Receiver<Cmd>, on_change: Callback) {
    STATE.with(|s| {
        *s.borrow_mut() = Some(State {
            matcher: Matcher::new(),
            on_change,
            tap: None,
            asked_access: false,
            warned_tap: false,
        })
    });
    let Some(run_loop) = CFRunLoop::current() else {
        tracing::error!("push-to-talk: no run loop on the listener thread");
        return;
    };
    // SAFETY: an immutable CoreFoundation constant.
    let mode = unsafe { kCFRunLoopDefaultMode };
    'outer: loop {
        let has_tap = with_state(|st| st.tap.is_some()).unwrap_or(false);
        // With a tap the run loop delivers its events for one slice; without one there is nothing
        // to run (the run loop would return at once), so just wait for a command.
        let first = if has_tap {
            CFRunLoop::run_in_mode(mode, SLICE.as_secs_f64(), false);
            rx.try_recv().ok()
        } else {
            match rx.recv_timeout(SLICE) {
                Ok(cmd) => Some(cmd),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        };
        let mut next = first;
        while let Some(cmd) = next {
            match cmd {
                Cmd::Stop => break 'outer,
                Cmd::Rebind(binding) => with_state(|st| st.rebind(binding, &run_loop)),
            };
            next = rx.try_recv().ok();
        }
        with_state(|st| {
            st.check_held();
            // Input Monitoring may have been allowed since the last try.
            if st.tap.is_none() && st.matcher.binding().is_some() {
                st.install(&run_loop);
            }
        });
    }
    with_state(|st| st.rebind(None, &run_loop));
    STATE.with(|s| s.borrow_mut().take());
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.borrow_mut().as_mut().map(f))
}

impl State {
    fn report(&self, change: Option<bool>) {
        if let Some(pressed) = change {
            (self.on_change)(pressed);
        }
    }

    fn rebind(&mut self, binding: Option<Binding>, run_loop: &CFRunLoop) {
        let change = self.matcher.set_binding(binding);
        self.report(change);
        let want_mouse = matches!(
            binding,
            Some(Binding {
                trigger: Trigger::Mouse(_),
                ..
            })
        );
        let keep = binding.is_some() && self.tap.as_ref().is_some_and(|t| t.mouse == want_mouse);
        if !keep {
            self.uninstall(run_loop);
            if binding.is_some() {
                self.install(run_loop);
            }
        }
    }

    /// Creates the tap for the current binding, if macOS allows it.
    fn install(&mut self, run_loop: &CFRunLoop) {
        let Some(binding) = self.matcher.binding() else {
            return;
        };
        if !has_input_access() {
            if !std::mem::replace(&mut self.asked_access, true) {
                tracing::warn!("push-to-talk needs Input Monitoring access; asking macOS");
                // Shows the system prompt the first time; afterwards the user has to allow it in
                // System Settings (the UI explains where).
                CGRequestListenEventAccess();
            }
            return;
        }
        let mouse = matches!(binding.trigger, Trigger::Mouse(_));
        let bit = |t: CGEventType| -> CGEventMask { 1 << t.0 };
        let mut mask =
            bit(CGEventType::KeyDown) | bit(CGEventType::KeyUp) | bit(CGEventType::FlagsChanged);
        if mouse {
            mask |= bit(CGEventType::OtherMouseDown) | bit(CGEventType::OtherMouseUp);
        }
        // SAFETY: `tap_callback` matches CGEventTapCallBack and uses no user info; the tap is
        // listen-only, so the callback cannot alter or drop events.
        let port = unsafe {
            CGEvent::tap_create(
                CGEventTapLocation::SessionEventTap,
                CGEventTapPlacement::TailAppendEventTap,
                CGEventTapOptions::ListenOnly,
                mask,
                Some(tap_callback),
                std::ptr::null_mut(),
            )
        };
        let Some(port) = port else {
            if !std::mem::replace(&mut self.warned_tap, true) {
                tracing::warn!("push-to-talk: could not create the event tap; retrying");
            }
            return;
        };
        let Some(source) = CFMachPort::new_run_loop_source(None, Some(&port), 0) else {
            if !std::mem::replace(&mut self.warned_tap, true) {
                tracing::warn!("push-to-talk: could not create the run loop source; retrying");
            }
            port.invalidate();
            return;
        };
        // SAFETY: an immutable CoreFoundation constant.
        run_loop.add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });
        CGEvent::tap_enable(&port, true);
        tracing::info!(mouse, "push-to-talk listener active");
        self.tap = Some(Tap {
            port,
            source,
            mouse,
        });
    }

    fn uninstall(&mut self, run_loop: &CFRunLoop) {
        if let Some(tap) = self.tap.take() {
            CGEvent::tap_enable(&tap.port, false);
            // SAFETY: an immutable CoreFoundation constant.
            run_loop.remove_source(Some(&tap.source), unsafe { kCFRunLoopDefaultMode });
            tap.source.invalidate();
            tap.port.invalidate();
        }
    }

    fn on_event(&mut self, ty: CGEventType, event: &CGEvent) {
        let input = match ty {
            CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
                if let Some(tap) = &self.tap {
                    CGEvent::tap_enable(&tap.port, true);
                }
                return;
            }
            CGEventType::KeyDown | CGEventType::KeyUp => {
                let kvk = keycode(event);
                let Some((scan, e0)) = to_scan(kvk) else {
                    return;
                };
                Input::Key {
                    scan,
                    e0,
                    down: ty == CGEventType::KeyDown,
                }
            }
            CGEventType::FlagsChanged => {
                let kvk = keycode(event);
                let (Some(flag), Some((scan, e0))) = (modifier_flag(kvk), to_scan(kvk)) else {
                    return;
                };
                Input::Key {
                    scan,
                    e0,
                    down: CGEvent::flags(Some(event)).0 & flag != 0,
                }
            }
            CGEventType::OtherMouseDown | CGEventType::OtherMouseUp => {
                let n =
                    CGEvent::integer_value_field(Some(event), CGEventField::MouseEventButtonNumber);
                let button = match n {
                    2 => MouseButton::Middle,
                    3 => MouseButton::Back,
                    4 => MouseButton::Forward,
                    _ => return,
                };
                Input::Mouse {
                    button,
                    down: ty == CGEventType::OtherMouseDown,
                }
            }
            _ => return,
        };
        let change = self.matcher.feed(input);
        self.report(change);
    }

    /// Ends a transmission whose trigger is no longer physically down.
    fn check_held(&mut self) {
        if !self.matcher.is_pressed() {
            return;
        }
        let held = self
            .matcher
            .binding()
            .is_none_or(|b| physically_down(b.trigger));
        if !held {
            let change = self.matcher.release();
            self.report(change);
        }
    }
}

fn keycode(event: &CGEvent) -> u16 {
    CGEvent::integer_value_field(Some(event), CGEventField::KeyboardEventKeycode) as u16
}

/// Whether the trigger is down right now. If its key cannot be determined, assume it is, rather
/// than cutting the user off.
fn physically_down(trigger: Trigger) -> bool {
    let state = CGEventSourceStateID::HIDSystemState;
    match trigger {
        Trigger::Key { scan, e0 } => {
            to_kvk(scan, e0).is_none_or(|kvk| CGEventSource::key_state(state, kvk))
        }
        Trigger::Mouse(button) => {
            let n = match button {
                MouseButton::Middle => 2,
                MouseButton::Back => 3,
                MouseButton::Forward => 4,
            };
            CGEventSource::button_state(state, CGMouseButton(n))
        }
    }
}

unsafe extern "C-unwind" fn tap_callback(
    _proxy: CGEventTapProxy,
    ty: CGEventType,
    event: NonNull<CGEvent>,
    _user_info: *mut c_void,
) -> *mut CGEvent {
    // SAFETY: the event is valid for the duration of the callback.
    let ev = unsafe { event.as_ref() };
    // The callback runs inside `run_in_mode` on the listener thread, while no state is borrowed.
    STATE.with(|s| {
        if let Ok(mut s) = s.try_borrow_mut() {
            if let Some(st) = s.as_mut() {
                st.on_event(ty, ev);
            }
        }
    });
    // Listen-only: the return value is ignored; the event goes on unchanged.
    event.as_ptr()
}
