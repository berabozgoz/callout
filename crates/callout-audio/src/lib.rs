//! Ses hattı: cihaz keşfi, yakalama/oynatma ve (ileride) Opus + karıştırıcı.
//!
//! # Gerçek zamanlı kural
//! `cpal` geri çağrıları ses sürücüsünün iş parçacığında çalışır. Orada
//! **yalnızca** halka tampona kopyalama yapılır: bellek ayırma, kilit, I/O,
//! günlükleme ve kodlama yasaktır. Bu kural ihlal edilirse çıtırtı kaçınılmazdır.
//!
//! Modüller: [`codec`] (Opus), [`resample`] (cihaz hızı ↔ 48 kHz), [`framer`] (20 ms çerçeve),
//! [`vad`] (konuşma algılama), [`jitter`] (gönderen başına jitter tamponu), [`mixer`] (karıştırıcı).
//! Loopback örneği: `cargo run -p callout-audio --example loopback`.

#![forbid(unsafe_code)]

pub mod codec;
pub mod framer;
pub mod jitter;
pub mod mixer;
pub mod resample;
pub mod vad;

use cpal::traits::{DeviceTrait, HostTrait};

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("ses cihazı bulunamadı")]
    NoDevice,
    #[error("cihazlar listelenemedi: {0}")]
    Devices(#[from] cpal::DevicesError),
    #[error("cihaz adı okunamadı: {0}")]
    DeviceName(#[from] cpal::DeviceNameError),
    #[error("varsayılan yapılandırma alınamadı: {0}")]
    DefaultConfig(#[from] cpal::DefaultStreamConfigError),
    #[error("akış kurulamadı: {0}")]
    BuildStream(#[from] cpal::BuildStreamError),
    #[error("akış başlatılamadı: {0}")]
    PlayStream(#[from] cpal::PlayStreamError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
}

/// Arayüze gösterilecek cihaz bilgisi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Sürücünün verdiği ad; aynı zamanda seçim anahtarı olarak kullanılır.
    pub name: String,
    pub direction: Direction,
    pub is_default: bool,
    pub sample_rate_hz: u32,
    pub channels: u16,
}

/// Varsayılan ana bilgisayardaki (Windows'ta WASAPI) tüm giriş ve çıkış cihazlarını listeler.
pub fn list_devices() -> Result<Vec<DeviceInfo>, AudioError> {
    let host = cpal::default_host();
    let default_in = host.default_input_device().and_then(|d| d.name().ok());
    let default_out = host.default_output_device().and_then(|d| d.name().ok());

    let mut out = Vec::new();
    for dev in host.input_devices()? {
        let name = dev.name()?;
        if let Ok(cfg) = dev.default_input_config() {
            out.push(DeviceInfo {
                is_default: default_in.as_deref() == Some(name.as_str()),
                name,
                direction: Direction::Input,
                sample_rate_hz: cfg.sample_rate().0,
                channels: cfg.channels(),
            });
        }
    }
    for dev in host.output_devices()? {
        let name = dev.name()?;
        if let Ok(cfg) = dev.default_output_config() {
            out.push(DeviceInfo {
                is_default: default_out.as_deref() == Some(name.as_str()),
                name,
                direction: Direction::Output,
                sample_rate_hz: cfg.sample_rate().0,
                channels: cfg.channels(),
            });
        }
    }
    Ok(out)
}

/// Ada göre cihaz bulur; `None` verilirse yönün varsayılan cihazı döner.
pub fn find_device(direction: Direction, name: Option<&str>) -> Result<cpal::Device, AudioError> {
    let host = cpal::default_host();
    let found = match (direction, name) {
        (Direction::Input, None) => host.default_input_device(),
        (Direction::Output, None) => host.default_output_device(),
        (Direction::Input, Some(n)) => host
            .input_devices()?
            .find(|d| d.name().ok().as_deref() == Some(n)),
        (Direction::Output, Some(n)) => host
            .output_devices()?
            .find(|d| d.name().ok().as_deref() == Some(n)),
    };
    found.ok_or(AudioError::NoDevice)
}
