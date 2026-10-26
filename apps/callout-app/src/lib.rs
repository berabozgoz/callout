//! Tauri kabuğu. Kural: burada ses ya da ağ mantığı yoktur; yalnızca
//! `callout-core` motoruna komut iletir, olaylarını `engine` adıyla webview'a yayar,
//! ayarları saklar, bas-konuş kısayolunu ve sistem tepsisini yönetir.

use callout_core::{Command, DeviceInfo, Direction, Engine, EngineConfig, Event};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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

struct AppState {
    engine: Engine,
    settings: Mutex<Settings>,
    data_dir: PathBuf,
}

impl AppState {
    fn save_settings(&self) -> Result<(), String> {
        let s = self.settings.lock().unwrap().clone();
        let path = self.data_dir.join("settings.json");
        std::fs::create_dir_all(&self.data_dir).map_err(|e| e.to_string())?;
        std::fs::write(path, serde_json::to_vec_pretty(&s).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    }
}

fn load_settings(data_dir: &PathBuf) -> Settings {
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
    if old.ptt_shortcut != settings.ptt_shortcut {
        register_ptt(&app, &settings.ptt_shortcut)?;
    }
    Ok(())
}

/// Motora komut: arayüz `Command`'ı JSON olarak (`{"type": "...", ...}`) gönderir.
#[tauri::command]
fn engine_command(state: State<AppState>, command: Command) {
    state.engine.send(command);
}

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

// ---------------------------------------------------------------------------
// Bas-konuş kısayolu
// ---------------------------------------------------------------------------

fn register_ptt(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    let gs = app.global_shortcut();
    gs.unregister_all().map_err(|e| e.to_string())?;
    if shortcut.trim().is_empty() {
        return Ok(());
    }
    gs.register(shortcut).map_err(|e| format!("kısayol '{shortcut}': {e}"))
}

// ---------------------------------------------------------------------------
// Kurulum
// ---------------------------------------------------------------------------

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

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
            // Veri klasörü: %APPDATA%/com.masatenisiligleri.callout
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
                    let _ = handle.emit("engine", &ev);
                }
            });

            if let Err(e) = register_ptt(app.handle(), &settings.ptt_shortcut) {
                tracing::warn!("{e}");
            }

            app.manage(AppState {
                engine,
                settings: Mutex::new(settings),
                data_dir,
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
            app_version
        ])
        .run(tauri::generate_context!())
        .expect("tauri başlatılamadı");
}

#[allow(dead_code)]
fn _assert_event_serializable(e: &Event) -> String {
    serde_json::to_string(e).unwrap_or_default()
}
