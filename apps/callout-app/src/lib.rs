//! Tauri shell. Rule: no audio or network logic lives here; it only
//! forwards commands to the `callout-core` engine, emits its events to the webview as `engine`,
//! stores settings, and manages the push-to-talk listener and the system tray.

use callout_core::{
    Command, ConnectionState, DeviceInfo, Direction, Engine, EngineConfig, Event, Member, RoomInfo,
    UserId,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, State};

/// User settings stored on disk (`<app-data>/settings.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub nickname: String,
    pub last_invite: String,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub push_to_talk: bool,
    /// `KeyboardEvent.code` names with modifiers or a mouse button, e.g. `KeyV`, `Ctrl+Space`,
    /// `Mouse4` (see `callout_input::Binding`); empty = not set. The key is not swallowed, so it
    /// also does what it does in the game; there is no default (any key clashes with some game,
    /// F9 pauses Dota 2), the UI asks for one when push-to-talk is turned on.
    pub ptt_shortcut: String,
    /// Noise suppression (RNNoise); on if missing from older settings files.
    pub noise_suppression: bool,
    /// Voice activity sensitivity in dB (0 = default, positive = picks up quieter speech).
    pub vad_sensitivity: f32,
    /// Automatic gain control on the microphone; on if missing from older settings files.
    pub agc: bool,
    /// UI language code (`en`, `tr`, …); `None` follows the system language.
    pub language: Option<String>,
    /// Rooms joined lately (newest first), for one-click rejoin. Rooms are persistent on the server.
    pub recent_rooms: Vec<RecentRoom>,
    /// Installer language (Windows LCID) already applied to `language`; a new install with a
    /// different choice applies it again, the same one does not override a later in-app change.
    pub installer_language_seen: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecentRoom {
    pub code: String,
    pub name: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            nickname: String::new(),
            last_invite: String::new(),
            input_device: None,
            output_device: None,
            push_to_talk: false,
            ptt_shortcut: String::new(),
            noise_suppression: true,
            vad_sensitivity: 0.0,
            agc: true,
            language: None,
            recent_rooms: Vec::new(),
            installer_language_seen: None,
        }
    }
}

/// A copy of the UI (webview) state that could be lost: the engine stays connected even if the
/// webview reloads, and on startup the UI fills itself from here via `engine_snapshot`.
#[derive(Debug, Clone, Serialize)]
struct Snapshot {
    connection: ConnectionState,
    user_id: Option<UserId>,
    room: Option<RoomInfo>,
    members: Vec<Member>,
    muted: bool,
    deafened: bool,
    /// Last audio pipeline start: `None` before any, `Some(None)` without a microphone.
    input_rate: Option<Option<u32>>,
}

impl Snapshot {
    fn new() -> Self {
        Self {
            connection: ConnectionState::Disconnected,
            user_id: None,
            room: None,
            members: Vec::new(),
            muted: false,
            deafened: false,
            input_rate: None,
        }
    }

    fn apply(&mut self, ev: &Event) {
        match ev {
            Event::ConnectionState { state } => {
                self.connection = *state;
                if *state == ConnectionState::Disconnected {
                    self.user_id = None;
                    self.room = None;
                    self.members.clear();
                }
            }
            Event::Connected { user_id } => self.user_id = Some(*user_id),
            Event::RoomUpdated { room, members } => {
                self.room = room.clone();
                self.members = members.clone();
            }
            Event::AudioStarted { input_rate, .. } => self.input_rate = Some(*input_rate),
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
struct AppliedAudio {
    input_device: Option<String>,
    output_device: Option<String>,
    noise_suppression: bool,
    vad_sensitivity: f32,
    agc: bool,
}

struct AppState {
    engine: Engine,
    snapshot: Mutex<Snapshot>,
    /// Push-to-talk is paused while a key is being captured in settings (pressing the old key
    /// there must not transmit).
    ptt_capture: AtomicBool,
    /// Push-to-talk listener; `None` if it could not start (push-to-talk then does nothing and
    /// the log says why).
    ptt: Option<callout_input::Listener>,
    settings: Mutex<Settings>,
    data_dir: PathBuf,
    /// Are we in a room? The push-to-talk listener receives input only while in a room.
    in_room: AtomicBool,
    /// Audio settings last sent to the engine. The settings screen applies a selection without
    /// saving it (so the sound test uses it); saving does not resend the same values.
    applied: Mutex<AppliedAudio>,
}

impl AppState {
    fn save_settings(&self) -> Result<(), String> {
        let s = self.settings.lock().unwrap().clone();
        let path = self.data_dir.join("settings.json");
        std::fs::create_dir_all(&self.data_dir).map_err(|e| e.to_string())?;
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&s).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }
}

fn load_settings(data_dir: &Path) -> Settings {
    std::fs::read(data_dir.join("settings.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Up to v0.1.13 every settings file was saved with the old default key `F9`, picked or not.
/// Where push-to-talk is off it was most likely never picked: clear it, so turning push-to-talk
/// on asks for a key. Returns whether anything changed.
fn forget_old_default_ptt_key(s: &mut Settings) -> bool {
    if !s.push_to_talk && s.ptt_shortcut == "F9" {
        s.ptt_shortcut.clear();
        return true;
    }
    false
}

/// The default window height fits the settings page without scrolling; on a short screen
/// (e.g. 1366×768) shrink it to the work area instead of letting it run under the taskbar.
fn fit_window_to_screen(w: &tauri::WebviewWindow) -> tauri::Result<()> {
    let Some(monitor) = w.current_monitor()? else {
        return Ok(());
    };
    let avail = monitor.work_area().size.height;
    let outer = w.outer_size()?;
    if outer.height > avail {
        let inner = w.inner_size()?;
        let height = inner.height.saturating_sub(outer.height - avail);
        w.set_size(tauri::PhysicalSize::new(inner.width, height))?;
        w.center()?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Commands exposed to the UI
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct DeviceDto {
    name: String,
    direction: &'static str,
    is_default: bool,
}

#[tauri::command]
fn list_audio_devices() -> Result<Vec<DeviceDto>, String> {
    callout_core::list_devices()
        .map(|v| {
            v.into_iter()
                .map(|d: DeviceInfo| DeviceDto {
                    name: d.name,
                    direction: match d.direction {
                        Direction::Input => "input",
                        Direction::Output => "output",
                    },
                    is_default: d.is_default,
                })
                .collect()
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(app: AppHandle, state: State<AppState>, settings: Settings) -> Result<(), String> {
    let old = state.settings.lock().unwrap().clone();
    *state.settings.lock().unwrap() = settings.clone();
    state.save_settings()?;
    let applied = state.applied.lock().unwrap().clone();
    if applied.input_device != settings.input_device {
        send_command(
            &state,
            Command::SetInputDevice {
                name: settings.input_device.clone(),
            },
        );
    }
    if applied.output_device != settings.output_device {
        send_command(
            &state,
            Command::SetOutputDevice {
                name: settings.output_device.clone(),
            },
        );
    }
    if applied.vad_sensitivity != settings.vad_sensitivity {
        send_command(
            &state,
            Command::SetVadSensitivity {
                db: settings.vad_sensitivity,
            },
        );
    }
    if applied.noise_suppression != settings.noise_suppression {
        send_command(
            &state,
            Command::SetNoiseSuppression {
                enabled: settings.noise_suppression,
            },
        );
    }
    if applied.agc != settings.agc {
        send_command(
            &state,
            Command::SetAgc {
                enabled: settings.agc,
            },
        );
    }
    state.engine.send(Command::SetPushToTalkMode {
        enabled: settings.push_to_talk,
    });
    if old.ptt_shortcut != settings.ptt_shortcut || old.push_to_talk != settings.push_to_talk {
        sync_ptt(&app)?;
    }
    Ok(())
}

/// Engine command: the UI sends a `Command` as JSON (`{"type": "...", ...}`).
#[tauri::command]
fn engine_command(state: State<AppState>, command: Command) {
    send_command(&state, command);
}

/// Forwards a command to the engine; updates the state the app keeps (snapshot, applied devices).
fn send_command(state: &AppState, command: Command) {
    {
        let mut snap = state.snapshot.lock().unwrap();
        match &command {
            Command::SetMuted { muted } => snap.muted = *muted,
            Command::SetDeafened { deafened } => snap.deafened = *deafened,
            Command::SetInputDevice { name } => {
                state.applied.lock().unwrap().input_device = name.clone()
            }
            Command::SetOutputDevice { name } => {
                state.applied.lock().unwrap().output_device = name.clone()
            }
            Command::SetNoiseSuppression { enabled } => {
                state.applied.lock().unwrap().noise_suppression = *enabled
            }
            Command::SetVadSensitivity { db } => {
                state.applied.lock().unwrap().vad_sensitivity = *db
            }
            Command::SetAgc { enabled } => state.applied.lock().unwrap().agc = *enabled,
            _ => {}
        }
    }
    state.engine.send(command);
}

/// Reads the engine's actual state when the UI opens (or reloads).
#[tauri::command]
fn engine_snapshot(state: State<AppState>) -> Snapshot {
    state.snapshot.lock().unwrap().clone()
}

/// When key capture in settings starts/ends: pauses push-to-talk meanwhile.
#[tauri::command]
fn ptt_capture(app: AppHandle, state: State<AppState>, active: bool) -> Result<(), String> {
    state.ptt_capture.store(active, Ordering::Relaxed);
    sync_ptt(&app)
}

/// Tray menu items, so the UI can relabel them in its language.
struct TrayItems {
    show: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

/// The UI owns the translations; it sends the tray labels whenever the language changes.
#[tauri::command]
fn set_tray_labels(app: AppHandle, show: String, quit: String) -> Result<(), String> {
    let Some(items) = app.try_state::<TrayItems>() else {
        return Ok(());
    };
    items.show.set_text(show).map_err(|e| e.to_string())?;
    items.quit.set_text(quit).map_err(|e| e.to_string())
}

/// `true` when running from an installation (the NSIS installer puts `uninstall.exe` next to the
/// app), `false` for the portable exe. The updater replaces only installed copies; a portable exe
/// would just get a second, installed copy, so the UI offers a download link instead.
#[tauri::command]
fn is_installed() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("uninstall.exe").exists()))
        .unwrap_or(false)
}

/// Opens the GitHub Releases page in the default browser (portable users update by hand).
#[tauri::command]
fn open_releases_page(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(
            "https://github.com/berabozgoz/callout/releases/latest",
            None::<&str>,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn app_version(app: AppHandle) -> String {
    // Packaged version (same source as the installer's: Cargo.toml, written from the tag).
    app.package_info().version.to_string()
}

// ---------------------------------------------------------------------------
// Push-to-talk
// ---------------------------------------------------------------------------

/// The listener observes the key without swallowing it, but it still sees every keystroke while
/// it has a binding. So the binding is set only while push-to-talk is on **and** we are in a room
/// (and no key is being captured); otherwise it is cleared and no input is received.
fn sync_ptt(app: &AppHandle) -> Result<(), String> {
    let Some(state) = app.try_state::<AppState>() else {
        return Ok(());
    };
    let (enabled, shortcut) = {
        let s = state.settings.lock().unwrap();
        (s.push_to_talk, s.ptt_shortcut.trim().to_string())
    };
    // Report invalid syntax to the user even outside a room.
    let binding = if shortcut.is_empty() {
        None
    } else {
        Some(
            shortcut
                .parse::<callout_input::Binding>()
                .map_err(|e| format!("shortcut '{shortcut}': {e}"))?,
        )
    };
    let active = enabled
        && state.in_room.load(Ordering::Relaxed)
        && !state.ptt_capture.load(Ordering::Relaxed);
    if let Some(listener) = &state.ptt {
        listener.set_binding(binding.filter(|_| active));
    }
    Ok(())
}

/// Starts the listener; its callback runs on the listener's own thread.
fn start_ptt_listener(app: &AppHandle) -> Option<callout_input::Listener> {
    let handle = app.clone();
    let started = callout_input::Listener::start(move |pressed| {
        if let Some(state) = handle.try_state::<AppState>() {
            state.engine.send(Command::PushToTalk { pressed });
        }
        let _ = handle.emit("ptt", pressed);
    });
    match started {
        Ok(listener) => Some(listener),
        Err(e) => {
            tracing::error!("{e}");
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

/// Log file: `%APPDATA%/io.github.berabozgoz.callout/logs/callout.log`.
/// Moved to `callout.old.log` when it exceeds 2 MB. Falls back to stderr if it cannot be opened.
fn open_log_file() -> Option<std::fs::File> {
    let dir = PathBuf::from(std::env::var_os("APPDATA")?)
        .join("io.github.berabozgoz.callout")
        .join("logs");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("callout.log");
    if std::fs::metadata(&path)
        .map(|m| m.len() > 2_000_000)
        .unwrap_or(false)
    {
        let _ = std::fs::rename(&path, dir.join("callout.old.log"));
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok()
}

/// The language picked in the NSIS installer (stored by the installer as a Windows LCID under
/// `HKCU\Software\berabozgoz\CallOut`, value "Installer Language"), mapped to a UI language code.
#[cfg(windows)]
fn installer_language() -> Option<(u32, &'static str)> {
    let key = windows_registry::CURRENT_USER
        .open(r"Software\berabozgoz\CallOut")
        .ok()?;
    let lcid: u32 = key
        .get_string("Installer Language")
        .ok()?
        .trim()
        .parse()
        .ok()?;
    let code = match lcid {
        1033 => "en",
        1055 => "tr",
        1031 => "de",
        1034 | 3082 => "es",
        1036 => "fr",
        1046 => "pt-BR",
        1049 => "ru",
        1025 => "ar",
        2052 => "zh-CN",
        1041 => "ja",
        1042 => "ko",
        _ => return None,
    };
    Some((lcid, code))
}

#[cfg(not(windows))]
fn installer_language() -> Option<(u32, &'static str)> {
    None
}

/// Panics go to the log as well, so a crash can be told apart from a forced close (and a panic on
/// the audio thread, which does not take the app down, still leaves a trace).
fn log_panics() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        tracing::error!(thread = thread.name().unwrap_or("?"), "panic: {info}");
        default(info);
    }));
}

pub fn run() {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    match open_log_file() {
        Some(file) => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .with_writer(Mutex::new(file))
            .init(),
        None => tracing_subscriber::fmt().with_env_filter(filter).init(),
    }
    log_panics();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "CallOut started");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        // Updates come from GitHub Releases (latest.json), signed with the key in the release workflow.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Data folder: %APPDATA%/io.github.berabozgoz.callout
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("callout-data"));
            let mut settings = load_settings(&data_dir);
            let mut save_now = forget_old_default_ptt_key(&mut settings);
            if let Some(w) = app.get_webview_window("main") {
                if let Err(e) = fit_window_to_screen(&w) {
                    tracing::warn!("window size: {e}");
                }
            }
            if let Some((lcid, code)) = installer_language() {
                if settings.installer_language_seen != Some(lcid) {
                    // The user picked this language in the installer: start the app in it.
                    settings.language = Some(code.to_string());
                    settings.installer_language_seen = Some(lcid);
                    save_now = true;
                }
            }

            let (engine, mut events) = Engine::start(EngineConfig {
                data_dir: data_dir.clone(),
                input_device: settings.input_device.clone(),
                output_device: settings.output_device.clone(),
                push_to_talk: settings.push_to_talk,
                noise_suppression: settings.noise_suppression,
                vad_sensitivity_db: settings.vad_sensitivity,
                agc: settings.agc,
                no_audio: false,
            });

            // Engine events → webview ("engine" channel).
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Some(ev) = events.recv().await {
                    if let Some(st) = handle.try_state::<AppState>() {
                        st.snapshot.lock().unwrap().apply(&ev);
                    }
                    if let Event::RoomUpdated { room, .. } = &ev {
                        // Sync push-to-talk only on room join/leave (when the value changes).
                        let in_room = room.is_some();
                        let changed = handle.try_state::<AppState>().is_some_and(|st| {
                            st.in_room.swap(in_room, Ordering::Relaxed) != in_room
                        });
                        if changed {
                            if let Err(e) = sync_ptt(&handle) {
                                tracing::warn!("{e}");
                            }
                        }
                    }
                    let _ = handle.emit("engine", &ev);
                }
            });

            app.manage(AppState {
                engine,
                applied: Mutex::new(AppliedAudio {
                    input_device: settings.input_device.clone(),
                    output_device: settings.output_device.clone(),
                    noise_suppression: settings.noise_suppression,
                    vad_sensitivity: settings.vad_sensitivity,
                    agc: settings.agc,
                }),
                settings: Mutex::new(settings),
                data_dir,
                in_room: AtomicBool::new(false),
                snapshot: Mutex::new(Snapshot::new()),
                ptt_capture: AtomicBool::new(false),
                ptt: start_ptt_listener(app.handle()),
            });
            if save_now {
                if let Err(e) = app.state::<AppState>().save_settings() {
                    tracing::warn!("could not save settings: {e}");
                }
            }

            // System tray: show / quit.
            let show = MenuItem::with_id(app, "show", "Show CallOut", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            app.manage(TrayItems {
                show: show.clone(),
                quit: quit.clone(),
            });
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("CallOut")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, ev| match ev.id().as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => {
                        if let Some(state) = app.try_state::<AppState>() {
                            state.engine.send(Command::Disconnect);
                            // Give the engine time to send the close frame (at most ~500 ms);
                            // if the process exits immediately the server only drops the connection on a ~10 s timeout.
                            std::thread::sleep(std::time::Duration::from_millis(700));
                        }
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, ev| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        ..
                    } = ev
                    {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // The close button hides the window to the tray; quitting is done from the tray menu.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_audio_devices,
            get_settings,
            save_settings,
            engine_command,
            engine_snapshot,
            ptt_capture,
            app_version,
            set_tray_labels,
            is_installed,
            open_releases_page
        ])
        .run(tauri::generate_context!())
        .expect("failed to start tauri");
}

#[allow(dead_code)]
fn _assert_event_serializable(e: &Event) -> String {
    serde_json::to_string(e).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{forget_old_default_ptt_key, Settings};

    #[test]
    fn old_default_ptt_key_is_forgotten_only_when_unused() {
        let mut s = Settings {
            ptt_shortcut: "F9".into(),
            ..Settings::default()
        };
        assert!(forget_old_default_ptt_key(&mut s));
        assert_eq!(s.ptt_shortcut, "");
        // In use with push-to-talk on: kept.
        let mut s = Settings {
            push_to_talk: true,
            ptt_shortcut: "F9".into(),
            ..Settings::default()
        };
        assert!(!forget_old_default_ptt_key(&mut s));
        assert_eq!(s.ptt_shortcut, "F9");
        // Any other key the user picked: kept.
        let mut s = Settings {
            ptt_shortcut: "F8".into(),
            ..Settings::default()
        };
        assert!(!forget_old_default_ptt_key(&mut s));
    }
}

#[cfg(all(test, windows))]
mod installer_language_tests {
    #[test]
    #[ignore = "reads this machine's registry"]
    fn print_installer_language() {
        println!("installer language: {:?}", super::installer_language());
    }
}
