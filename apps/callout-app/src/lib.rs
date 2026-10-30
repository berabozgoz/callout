//! Tauri kabuğu. Kural: burada ses ya da ağ mantığı yoktur; yalnızca
//! `callout-core` motoruna komut iletir, olaylarını `engine` adıyla webview'a yayar,
//! ayarları saklar, bas-konuş kısayolunu ve sistem tepsisini yönetir.

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

/// Diskte saklanan kullanıcı ayarları (`<app-data>/settings.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub nickname: String,
    pub last_invite: String,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub push_to_talk: bool,
    /// Tauri kısayol sözdizimi, ör. `F9`, `Ctrl+Space`, `Alt+Z`.
    pub ptt_shortcut: String,
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
        }
    }
}

/// Arayüzün (webview) kaybolabilecek durumunun kopyası: webview yenilense bile motor bağlı kalır,
/// arayüz açılışta `engine_snapshot` ile kendini buradan doldurur.
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

struct AppState {
    engine: Engine,
    snapshot: Mutex<Snapshot>,
    /// Ayarlarda tuş yakalama sürerken kısayol bırakılır (yoksa tuş arayüze ulaşmaz).
    ptt_capture: AtomicBool,
    settings: Mutex<Settings>,
    data_dir: PathBuf,
    /// Odada mıyız? Bas-konuş kısayolu yalnızca odadayken sistem genelinde tutulur.
    in_room: AtomicBool,
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
// Arayüze açılan komutlar
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
    if old.input_device != settings.input_device {
        state.engine.send(Command::SetInputDevice {
            name: settings.input_device.clone(),
        });
    }
    if old.output_device != settings.output_device {
        state.engine.send(Command::SetOutputDevice {
            name: settings.output_device.clone(),
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

/// Motora komut: arayüz `Command`'ı JSON olarak (`{"type": "...", ...}`) gönderir.
#[tauri::command]
fn engine_command(state: State<AppState>, command: Command) {
    {
        let mut snap = state.snapshot.lock().unwrap();
        match &command {
            Command::SetMuted { muted } => snap.muted = *muted,
            Command::SetDeafened { deafened } => snap.deafened = *deafened,
            _ => {}
        }
    }
    state.engine.send(command);
}

/// Arayüz açılırken (ya da yenilenince) motorun gerçek durumunu okur.
#[tauri::command]
fn engine_snapshot(state: State<AppState>) -> Snapshot {
    state.snapshot.lock().unwrap().clone()
}

/// Ayarlarda tuş yakalama başlarken/biterken: bas-konuş kısayolunu geçici bırakır.
#[tauri::command]
fn ptt_capture(app: AppHandle, state: State<AppState>, active: bool) -> Result<(), String> {
    state.ptt_capture.store(active, Ordering::Relaxed);
    sync_ptt(&app)
}

#[tauri::command]
fn app_version(app: AppHandle) -> String {
    // Paketlenen sürüm (kurulum dosyasındakiyle aynı kaynak: Cargo.toml → etiketten yazılır).
    app.package_info().version.to_string()
}

// ---------------------------------------------------------------------------
// Bas-konuş kısayolu
// ---------------------------------------------------------------------------

/// Kısayol sistem genelinde tuşu tüketir (düz "T" gibi bir tuş başka uygulamalarda yazılamaz).
/// Bu yüzden yalnızca bas-konuş açıkken **ve** odadayken kayıtlı tutulur; aksi halde bırakılır.
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
    // Geçersiz sözdizimini oda dışında da kullanıcıya bildir.
    shortcut
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map_err(|e| format!("kısayol '{shortcut}': {e}"))?;
    if enabled
        && state.in_room.load(Ordering::Relaxed)
        && !state.ptt_capture.load(Ordering::Relaxed)
    {
        gs.register(shortcut.as_str())
            .map_err(|e| format!("kısayol '{shortcut}': {e}"))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Kurulum
// ---------------------------------------------------------------------------

/// Günlük dosyası: `%APPDATA%/io.github.berabozgoz.callout/logs/callout.log`.
/// 2 MB'ı aşarsa `callout.old.log`'a taşınır. Açılamazsa stderr'e düşülür.
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
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "CallOut başladı");

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
                    // Hangi tuş olursa olsun tek kayıtlı kısayol bas-konuştur.
                    let pressed = event.state() == ShortcutState::Pressed;
                    if let Some(state) = app.try_state::<AppState>() {
                        state.engine.send(Command::PushToTalk { pressed });
                    }
                    let _ = app.emit("ptt", pressed);
                })
                .build(),
        )
        .setup(|app| {
            // Veri klasörü: %APPDATA%/io.github.berabozgoz.callout
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
                no_audio: false,
            });

            // Motor olayları → webview ("engine" kanalı).
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Some(ev) = events.recv().await {
                    if let Some(st) = handle.try_state::<AppState>() {
                        st.snapshot.lock().unwrap().apply(&ev);
                    }
                    if let Event::RoomUpdated { room, .. } = &ev {
                        // Yalnızca odaya giriş/çıkışta (değer değişince) kısayolu eşitle.
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
                settings: Mutex::new(settings),
                data_dir,
                in_room: AtomicBool::new(false),
                snapshot: Mutex::new(Snapshot::new()),
                ptt_capture: AtomicBool::new(false),
            });

            // Sistem tepsisi: göster / çık.
            let show = MenuItem::with_id(app, "show", "CallOut'u göster", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Çık", true, None::<&str>)?;
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
                            // Motorun kapatma çerçevesini göndermesine zaman ver (en çok ~500 ms);
                            // süreç hemen çıkarsa sunucu bağlantıyı ~10 sn sonra zaman aşımıyla siler.
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
            // Kapat düğmesi pencereyi tepsiye gizler; çıkış tepsi menüsünden.
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
        .expect("tauri başlatılamadı");
}

#[allow(dead_code)]
fn _assert_event_serializable(e: &Event) -> String {
    serde_json::to_string(e).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use tauri_plugin_global_shortcut::Shortcut;

    /// Arayüzün tuş yakalama alanı `KeyboardEvent.code` adlarını gönderir; hepsi kısayol
    /// kütüphanesince çözümlenmeli.
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
            assert!(s.parse::<Shortcut>().is_ok(), "çözümlenemedi: {s}");
        }
    }
}
