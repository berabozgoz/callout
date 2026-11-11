//! Tauri shell. Rule: no audio or network logic lives here; it only
//! forwards commands to the `callout-core` engine, emits its events to the webview as `engine`,
//! stores settings, and manages the push-to-talk listener and the system tray.

use callout_core::{
    Command, ConnectionState, DeviceInfo, Direction, Engine, EngineConfig, Event, JoinSound,
    Member, RoomInfo, UserId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    /// Automatic gain control on the microphone; off by default (see `agc_default_off_seen`).
    pub agc: bool,
    /// The one-time switch-off of the old AGC default has been applied (see
    /// `turn_off_old_agc_default`).
    pub agc_default_off_seen: bool,
    /// UI language code (`en`, `tr`, …); `None` follows the system language.
    pub language: Option<String>,
    /// Rooms joined lately (newest first), for one-click rejoin. Rooms are persistent on the server.
    pub recent_rooms: Vec<RecentRoom>,
    /// Installer language (Windows LCID) already applied to `language`; a new install with a
    /// different choice applies it again, the same one does not override a later in-app change.
    pub installer_language_seen: Option<u32>,
    /// Volumes set for other players (gain, 1.0 = normal), by nickname: the protocol has no stable
    /// user id (a `user_id` lasts one connection) until accounts come in Phase 3. Only values other
    /// than 1.0 are kept. Owned by the app: written when a slider moves, never taken from the UI's
    /// `save_settings` copy (which may be older).
    pub user_volumes: BTreeMap<String, f32>,
    /// Sound when someone joins or leaves the room (and when we join one).
    pub join_sound: JoinSound,
    /// Echo cancellation on the microphone; on if missing from older settings files.
    pub echo_cancellation: bool,
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
            agc: false,
            // `false` so that older files (which lack the field) get the one-time switch-off.
            agc_default_off_seen: false,
            language: None,
            recent_rooms: Vec::new(),
            installer_language_seen: None,
            user_volumes: BTreeMap::new(),
            echo_cancellation: true,
            join_sound: JoinSound::default(),
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
    echo_cancellation: bool,
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

/// v0.1.14–v0.1.16 had AGC on by default with a tuning that players found too loud (CS2 field
/// test, 4 Nov 2026). Turn it off once for settings files from those versions; a player who
/// switches it on again afterwards keeps it. Returns whether anything changed.
fn turn_off_old_agc_default(s: &mut Settings) -> bool {
    if s.agc_default_off_seen {
        return false;
    }
    s.agc = false;
    s.agc_default_off_seen = true;
    true
}

/// Stores the volume set for `nickname` (removed when back at normal). Returns whether the stored
/// map changed.
fn remember_volume(volumes: &mut BTreeMap<String, f32>, nickname: &str, gain: f32) -> bool {
    let gain = (gain.clamp(0.0, 4.0) * 100.0).round() / 100.0;
    if (gain - 1.0).abs() < 0.005 {
        volumes.remove(nickname).is_some()
    } else {
        volumes.insert(nickname.to_string(), gain) != Some(gain)
    }
}

/// Saved volumes to send to the engine for the members of a room (not for ourselves). Sent on every
/// room update: a `user_id` is new after each join or reconnect, and resending is harmless.
fn saved_volumes_for(
    members: &[Member],
    me: Option<UserId>,
    volumes: &BTreeMap<String, f32>,
) -> Vec<(UserId, f32)> {
    members
        .iter()
        .filter(|m| Some(m.user_id) != me)
        .filter_map(|m| volumes.get(&m.nickname).map(|g| (m.user_id, *g)))
        .collect()
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
fn save_settings(
    app: AppHandle,
    state: State<AppState>,
    mut settings: Settings,
) -> Result<(), String> {
    let old = state.settings.lock().unwrap().clone();
    // Internal flags are the app's, not the UI's: keep them even if the UI sent an object
    // without them (they would deserialize to their defaults and redo the one-time changes).
    settings.installer_language_seen = old.installer_language_seen;
    settings.agc_default_off_seen = old.agc_default_off_seen;
    settings.user_volumes = old.user_volumes;
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
    if applied.echo_cancellation != settings.echo_cancellation {
        send_command(
            &state,
            Command::SetEchoCancellation {
                enabled: settings.echo_cancellation,
            },
        );
    }
    if old.join_sound != settings.join_sound {
        state.engine.send(Command::SetJoinSound {
            sound: settings.join_sound,
        });
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
    if let Command::SetUserVolume { user_id, gain } = &command {
        let nickname = (state.snapshot.lock().unwrap().members.iter())
            .find(|m| m.user_id == *user_id)
            .map(|m| m.nickname.clone());
        if let Some(nickname) = nickname {
            let changed = remember_volume(
                &mut state.settings.lock().unwrap().user_volumes,
                &nickname,
                *gain,
            );
            if changed {
                if let Err(e) = state.save_settings() {
                    tracing::warn!("could not save settings: {e}");
                }
            }
        }
    }
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
            Command::SetEchoCancellation { enabled } => {
                state.applied.lock().unwrap().echo_cancellation = *enabled
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
#[cfg(not(target_os = "macos"))]
#[tauri::command]
fn is_installed() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("uninstall.exe").exists()))
        .unwrap_or(false)
}

/// macOS: `true` inside an app bundle that is not on a mounted disk image. Started straight from
/// the DMG the bundle is read-only, so the updater could not replace it; the UI offers the
/// download link instead (and the user should drag CallOut to Applications first).
#[cfg(target_os = "macos")]
#[tauri::command]
fn is_installed() -> bool {
    std::env::current_exe().is_ok_and(|exe| {
        let p = exe.to_string_lossy();
        p.contains(".app/Contents/MacOS/") && !p.starts_with("/Volumes/")
    })
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

/// `false` while macOS has not allowed CallOut to observe keystrokes in other apps (Input
/// Monitoring); push-to-talk cannot work until the user allows it. Always `true` on Windows.
#[tauri::command]
fn ptt_input_access() -> bool {
    callout_input::has_input_access()
}

/// Asks macOS for Input Monitoring (so CallOut is listed there) and opens System Settings →
/// Privacy & Security → Input Monitoring. Does nothing elsewhere.
#[tauri::command]
fn open_input_monitoring_settings(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_opener::OpenerExt;
        if callout_input::request_input_access() {
            return Ok(());
        }
        app.opener()
            .open_url(
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
                None::<&str>,
            )
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Ok(())
    }
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

/// Log file: `%APPDATA%/io.github.berabozgoz.callout/logs/callout.log` on Windows,
/// `~/Library/Logs/io.github.berabozgoz.callout/callout.log` on macOS.
/// Moved to `callout.old.log` when it exceeds 2 MB. Falls back to stderr if it cannot be opened.
fn open_log_file() -> Option<std::fs::File> {
    let dir = log_dir()?;
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

#[cfg(not(target_os = "macos"))]
fn log_dir() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("APPDATA")?)
            .join("io.github.berabozgoz.callout")
            .join("logs"),
    )
}

#[cfg(target_os = "macos")]
fn log_dir() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("HOME")?)
            .join("Library/Logs")
            .join("io.github.berabozgoz.callout"),
    )
}

// ---------------------------------------------------------------------------
// Sending the log to the server's operator
// ---------------------------------------------------------------------------

/// How much of the end of the log goes out (the server takes up to 1 MiB).
const LOG_UPLOAD_BYTES: usize = 1_000_000;

/// Sends the end of this app's log, cleaned (`scrub_log`), to the operator of the server we
/// are connected to. Only on the user's click after the consent text in the UI; the answer
/// arrives as `Event::LogUpload`.
#[tauri::command]
fn send_log(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let path = log_dir().ok_or("no log folder")?.join("callout.log");
    let bytes = std::fs::read(&path).map_err(|e| format!("read log: {e}"))?;
    let text = scrub_log(&String::from_utf8_lossy(&bytes));
    state.engine.send(Command::UploadLog {
        app_version: app.package_info().version.to_string(),
        log: tail_lines(&text, LOG_UPLOAD_BYTES).to_string(),
    });
    Ok(())
}

/// The last `max` bytes of `text`, starting at a line.
fn tail_lines(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut start = text.len() - max;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    match text[start..].find('\n') {
        Some(nl) => &text[start + nl + 1..],
        None => &text[start..],
    }
}

/// Removes what the log could reveal beyond CallOut itself: the user name in paths
/// (`C:\Users\<name>\…`, also with doubled or forward slashes, as in panic messages; macOS
/// `/Users/<name>/…`) and long
/// hex strings (the server's certificate fingerprint in an invite link).
fn scrub_log(text: &str) -> String {
    mask_hex_runs(&mask_user_dirs(text))
}

fn mask_user_dirs(text: &str) -> String {
    let is_sep = |c: u8| c == b'\\' || c == b'/';
    // ASCII lowercasing keeps every byte offset, so positions carry over to `text`.
    let lower = text.to_ascii_lowercase();
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let (mut copied, mut from) = (0, 0);
    while let Some(pos) = lower[from..].find("users").map(|p| p + from) {
        from = pos + 5;
        let mut j = from;
        while j < b.len() && is_sep(b[j]) {
            j += 1;
        }
        if pos == 0 || !is_sep(b[pos - 1]) || j == from {
            continue;
        }
        let name = j;
        while j < b.len() && !is_sep(b[j]) && !b"\t\r\n\"':".contains(&b[j]) {
            j += 1;
        }
        if j > name {
            out.push_str(&text[copied..name]);
            out.push_str("<user>");
            copied = j;
            from = j;
        }
    }
    out.push_str(&text[copied..]);
    out
}

/// Hex runs of 32+ digits become `<fingerprint>`; shorter ones (sequence numbers…) stay.
fn mask_hex_runs(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if run.len() >= 32 {
            out.push_str("<fingerprint>");
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for c in text.chars() {
        if c.is_ascii_hexdigit() {
            run.push(c);
        } else {
            flush(&mut run, &mut out);
            out.push(c);
        }
    }
    flush(&mut run, &mut out);
    out
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
            // Data folder: %APPDATA%/io.github.berabozgoz.callout on Windows,
            // ~/Library/Application Support/io.github.berabozgoz.callout on macOS.
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("callout-data"));
            let mut settings = load_settings(&data_dir);
            let mut save_now = forget_old_default_ptt_key(&mut settings);
            save_now |= turn_off_old_agc_default(&mut settings);
            if let Some(w) = app.get_webview_window("main") {
                if let Err(e) = fit_window_to_screen(&w) {
                    tracing::warn!("window size: {e}");
                }
                // Diagnostic builds only (feature `devtools`): show the UI's console and errors.
                #[cfg(feature = "devtools")]
                w.open_devtools();
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
                join_sound: settings.join_sound,
                echo_cancellation: settings.echo_cancellation,
                no_audio: false,
            });

            // Engine events → webview ("engine" channel).
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Some(ev) = events.recv().await {
                    if let Some(st) = handle.try_state::<AppState>() {
                        let me = {
                            let mut snap = st.snapshot.lock().unwrap();
                            snap.apply(&ev);
                            snap.user_id
                        };
                        if let Event::RoomUpdated { members, .. } = &ev {
                            let saved = saved_volumes_for(
                                members,
                                me,
                                &st.settings.lock().unwrap().user_volumes,
                            );
                            for (user_id, gain) in saved {
                                st.engine.send(Command::SetUserVolume { user_id, gain });
                            }
                        }
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
                    echo_cancellation: settings.echo_cancellation,
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
                        disconnect_before_exit(app);
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
            open_releases_page,
            send_log,
            ptt_input_access,
            open_input_monitoring_settings
        ])
        .build(tauri::generate_context!())
        .expect("failed to start tauri")
        .run(|app, event| match event {
            // Every way out ends here, also those that skip the tray menu: Cmd+Q, the app menu
            // and the Dock's Quit on macOS, logging out.
            tauri::RunEvent::Exit => disconnect_before_exit(app),
            // macOS: the window hides on close but the app keeps running; clicking the Dock icon
            // brings it back (on Windows the tray icon does that).
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            _ => {}
        });
}

/// Leaves the server politely before the process ends: without the close frame the server keeps
/// the connection (and the user in the room) until a ~10 s timeout. Runs once, whichever way the
/// app is quit.
fn disconnect_before_exit(app: &AppHandle) {
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.swap(true, Ordering::Relaxed) {
        return;
    }
    if let Some(state) = app.try_state::<AppState>() {
        state.engine.send(Command::Disconnect);
        // Give the engine time to send the close frame (at most ~500 ms).
        std::thread::sleep(std::time::Duration::from_millis(700));
    }
}

#[allow(dead_code)]
fn _assert_event_serializable(e: &Event) -> String {
    serde_json::to_string(e).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        forget_old_default_ptt_key, remember_volume, saved_volumes_for, scrub_log, tail_lines,
        turn_off_old_agc_default, Settings,
    };
    use callout_core::Member;
    use std::collections::BTreeMap;

    fn member(user_id: u32, nickname: &str) -> Member {
        Member {
            user_id,
            nickname: nickname.into(),
            muted: false,
            deafened: false,
        }
    }

    #[test]
    fn volumes_are_remembered_by_nickname() {
        let mut v = BTreeMap::new();
        assert!(remember_volume(&mut v, "kuzen", 0.55));
        assert!(
            !remember_volume(&mut v, "kuzen", 0.551),
            "same value after rounding"
        );
        assert_eq!(v.get("kuzen"), Some(&0.55));
        assert!(
            remember_volume(&mut v, "kuzen", 1.0),
            "back to normal removes it"
        );
        assert!(v.is_empty());
        assert!(!remember_volume(&mut v, "kuzen", 1.0));
        remember_volume(&mut v, "loud", 9.0);
        assert_eq!(v.get("loud"), Some(&4.0), "clamped like the engine");
    }

    #[test]
    fn saved_volumes_apply_to_others_in_the_room() {
        let mut v = BTreeMap::new();
        v.insert("kuzen".to_string(), 0.5);
        v.insert("bera".to_string(), 1.5);
        let members = [member(7, "bera"), member(8, "kuzen"), member(9, "cocart")];
        // Ourselves (7, even with a matching name) and players without a saved value are skipped.
        assert_eq!(saved_volumes_for(&members, Some(7), &v), vec![(8, 0.5)]);
    }

    #[test]
    fn settings_files_load_with_and_without_volumes() {
        let s: Settings = serde_json::from_str(r#"{"nickname":"bera"}"#).unwrap();
        assert!(s.user_volumes.is_empty());
        assert_eq!(
            s.join_sound,
            callout_core::JoinSound::Chime,
            "older files get the default"
        );
        assert!(
            s.echo_cancellation,
            "echo cancellation is on for older files"
        );
        let s: Settings =
            serde_json::from_str(r#"{"nickname":"bera","echo_cancellation":false}"#).unwrap();
        assert!(!s.echo_cancellation, "a saved choice is kept");
        let s: Settings = serde_json::from_str(r#"{"user_volumes":{"kuzen":0.7}}"#).unwrap();
        assert_eq!(s.user_volumes.get("kuzen"), Some(&0.7));
    }

    #[test]
    fn old_agc_default_is_turned_off_once() {
        // A settings file from v0.1.14–v0.1.16: `agc: true`, no `agc_default_off_seen`.
        let mut s: Settings = serde_json::from_str(r#"{"nickname":"bera","agc":true}"#).unwrap();
        assert!(turn_off_old_agc_default(&mut s));
        assert!(!s.agc && s.agc_default_off_seen);
        // Switched on again by the player later: kept.
        s.agc = true;
        assert!(!turn_off_old_agc_default(&mut s));
        assert!(s.agc);
        // A fresh install starts with it off; the flag is set on first start.
        let mut s = Settings::default();
        assert!(!s.agc);
        turn_off_old_agc_default(&mut s);
        assert!(!s.agc && s.agc_default_off_seen);
        // A file saved after the switch-off keeps the player's choice.
        let mut s: Settings =
            serde_json::from_str(r#"{"agc":true,"agc_default_off_seen":true}"#).unwrap();
        assert!(!turn_off_old_agc_default(&mut s));
        assert!(s.agc);
    }

    #[test]
    fn user_names_and_fingerprints_are_masked() {
        let log = concat!(
            "panic at C:\\Users\\Bera Bozgöz\\AppData\\x.rs:1\n",
            "path=\"C:\\\\Users\\\\bera\\\\AppData\" also c:/users/bera/Desktop\n",
            "invite callout://1.2.3.4:4433#b759212b66bd0a423dbed978e0bde842b4ca0d65a5b96fea746063848e8424a4\n",
            "mac panic at /Users/bera/Library/Logs/callout.log\n",
            "users=3 rtt_ms=40 seq=deadbeef\n",
        );
        let s = scrub_log(log);
        assert!(!s.contains("Bozgöz") && !s.contains("bera"), "{s}");
        assert!(s.contains(r"C:\Users\<user>\AppData\x.rs"), "{s}");
        assert!(s.contains(r#"path="C:\\Users\\<user>\\AppData""#), "{s}");
        assert!(s.contains("c:/users/<user>/Desktop"), "{s}");
        assert!(s.contains("/Users/<user>/Library/Logs"), "{s}");
        assert!(s.contains("callout://1.2.3.4:4433#<fingerprint>"), "{s}");
        // Words and short hex values stay as they are.
        assert!(s.contains("users=3 rtt_ms=40 seq=deadbeef"), "{s}");
    }

    #[test]
    fn tail_starts_at_a_line() {
        let text = "line one\nline two\nline three\n";
        assert_eq!(tail_lines(text, 100), text);
        assert_eq!(tail_lines(text, 14), "line three\n");
        assert_eq!(tail_lines("ççç\nabc", 5), "abc");
    }

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
