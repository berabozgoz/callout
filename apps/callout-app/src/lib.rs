//! Tauri kabuğu. Kural: burada ses ya da ağ mantığı yoktur; yalnızca
//! `callout-core` komutlarını arayüze açar ve olayları iletir.

use callout_core::{DeviceInfo, Direction};
use serde::Serialize;

/// Arayüze giden cihaz kaydı (JSON).
#[derive(Serialize)]
struct DeviceDto {
    name: String,
    direction: &'static str,
    is_default: bool,
    sample_rate_hz: u32,
    channels: u16,
}

impl From<DeviceInfo> for DeviceDto {
    fn from(d: DeviceInfo) -> Self {
        Self {
            name: d.name,
            direction: match d.direction {
                Direction::Input => "input",
                Direction::Output => "output",
            },
            is_default: d.is_default,
            sample_rate_hz: d.sample_rate_hz,
            channels: d.channels,
        }
    }
}

#[tauri::command]
fn list_audio_devices() -> Result<Vec<DeviceDto>, String> {
    callout_core::list_devices()
        .map(|v| v.into_iter().map(DeviceDto::from).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // İkinci kopya açılırsa mevcut pencereyi öne getir.
            use tauri::Manager;
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![list_audio_devices, app_version])
        .run(tauri::generate_context!())
        .expect("tauri başlatılamadı");
}
