//! Tauri shell. Rule: no audio or network logic lives here; it only
//! forwards commands to the `callout-core` engine, emits its events to the webview as `engine`,
//! stores settings, and manages the push-to-talk shortcut and the system tray.

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
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// User settings stored on disk (`<app-data>/settings.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub nickname: String,
    pub last_invite: String,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub push_to_talk: bool,
    /// Tauri shortcut syntax, e.g. `F9`, `Ctrl+Space`, `Alt+Z`.
    pub ptt_shortcut: String,
    /// Noise suppression (RNNoise); on if missing from older settings files.
    pub noise_suppression: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            nickname: String::new(),
            last_invite: String::new(),
            input_device: None,
            output_device: None,
            push_to_talk: false,
            ptt_shortcut: "F9".into(),
            noise_suppression: true,
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
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
struct AppliedAudio {
    input_device: Option<String>,
    output_device: Option<String>,
    noise_suppression: bool,
}

struct AppState {
    engine: Engine,
    snapshot: Mutex<Snapshot>,
    /// The shortcut is released while capturing a key in settings (otherwise the key never reaches the UI).
    ptt_capture: AtomicBool,
    settings: Mutex<Settings>,
    data_dir: PathBuf,
    /// Are we in a room? The push-to-talk shortcut is held system-wide only while in a room.
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
    if applied.noise_suppression != settings.noise_suppression {
        send_command(
            &state,
            Command::SetNoiseSuppression {
                enabled: settings.noise_suppression,
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

/// When key capture in settings starts/ends: temporarily releases the push-to-talk shortcut.
#[tauri::command]
fn ptt_capture(app: AppHandle, state: State<AppState>, active: bool) -> Result<(), String> {
    state.ptt_capture.store(active, Ordering::Relaxed);
    sync_ptt(&app)
}

#[tauri::command]
fn app_version(app: AppHandle) -> String {
    // Packaged version (same source as the installer's: Cargo.toml, written from the tag).
    app.package_info().version.to_string()
}

// ---------------------------------------------------------------------------
// Push-to-talk shortcut
// ---------------------------------------------------------------------------

/// The shortcut consumes the key system-wide (a plain key like "T" cannot be typed in other apps).
/// So it stays registered only while push-to-talk is on **and** we are in a room; otherwise it is released.
fn sync_ptt(app: &AppHandle) -> Result<(), String> {
    let Some(state) = app.try_state::<AppState>() else {
        return Ok(());
    };
    let (enabled, shortcut) = {
        let s = state.settings.lock().unwrap();
        (s.push_to_talk, s.ptt_shortcut.trim().to_string())
    };
    let gs = app.global_shortcut();
    gs.unregister_all().map_err(|e| e.to_string())?;
    if shortcut.is_empty() {
        return Ok(());
    }
    // Report invalid syntax to the user even outside a room.
    shortcut
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map_err(|e| format!("shortcut '{shortcut}': {e}"))?;
    if enabled
        && state.in_room.load(Ordering::Relaxed)
        && !state.ptt_capture.load(Ordering::Relaxed)
    {
        gs.register(shortcut.as_str())
            .map_err(|e| format!("shortcut '{shortcut}': {e}"))?;
    }
    Ok(())
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
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "CallOut started");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    // Whatever the key, the only registered shortcut is push-to-talk.
                    let pressed = event.state() == ShortcutState::Pressed;
                    if let Some(state) = app.try_state::<AppState>() {
                        state.engine.send(Command::PushToTalk { pressed });
                    }
                    let _ = app.emit("ptt", pressed);
                })
                .build(),
        )
        .setup(|app| {
            // Data folder: %APPDATA%/io.github.berabozgoz.callout
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("callout-data"));
            let settings = load_settings(&data_dir);

            let (engine, mut events) = Engine::start(EngineConfig {
                data_dir: data_dir.clone(),
                input_device: settings.input_device.clone(),
                output_device: settings.output_device.clone(),
                push_to_talk: settings.push_to_talk,
                noise_suppression: settings.noise_suppression,
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
                        // Sync the shortcut only on room join/leave (when the value changes).
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
                }),
                settings: Mutex::new(settings),
                data_dir,
                in_room: AtomicBool::new(false),
                snapshot: Mutex::new(Snapshot::new()),
                ptt_capture: AtomicBool::new(false),
            });

            // System tray: show / quit.
            let show = MenuItem::with_id(app, "show", "Show CallOut", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
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
            app_version
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
    use tauri_plugin_global_shortcut::Shortcut;

    /// The UI's key capture field sends `KeyboardEvent.code` names; the shortcut library must
    /// parse all of them.
    #[test]
    fn captured_key_codes_parse() {
        for s in [
            "F9",
            "F4",
            "Ctrl+F4",
            "KeyT",
            "Digit9",
            "Ctrl+KeyT",
            "Ctrl+Alt+KeyZ",
            "Ctrl+Shift+Digit1",
            "Alt+ArrowUp",
            "Space",
            "Ctrl+Space",
            "Numpad5",
            "Super+KeyV",
            "Backquote",
        ] {
            assert!(s.parse::<Shortcut>().is_ok(), "failed to parse: {s}");
        }
    }
}
