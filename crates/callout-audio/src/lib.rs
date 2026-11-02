//! Audio pipeline: device discovery, capture/playback and (later) Opus + mixer.
//!
//! # Real-time rule
//! `cpal` callbacks run on the audio driver's thread. They do **only** a copy
//! into the ring buffer: memory allocation, locks, I/O, logging and encoding
//! are forbidden. Break this rule and crackling is inevitable.
//!
//! Modules: [`codec`] (Opus), [`resample`] (device rate ↔ 48 kHz), [`framer`] (20 ms frames),
//! [`vad`] (voice activity detection), [`jitter`] (per-sender jitter buffer), [`mixer`] (mixer).
//! Loopback example: `cargo run -p callout-audio --example loopback`.

#![forbid(unsafe_code)]

pub mod codec;
pub mod denoise;
pub mod framer;
pub mod jitter;
pub mod mixer;
pub mod resample;
pub mod vad;

use cpal::traits::{DeviceTrait, HostTrait};

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("audio device not found")]
    NoDevice,
    #[error("could not list devices: {0}")]
    Devices(#[from] cpal::DevicesError),
    #[error("could not read device name: {0}")]
    DeviceName(#[from] cpal::DeviceNameError),
    #[error("could not get default config: {0}")]
    DefaultConfig(#[from] cpal::DefaultStreamConfigError),
    #[error("could not build stream: {0}")]
    BuildStream(#[from] cpal::BuildStreamError),
    #[error("could not start stream: {0}")]
    PlayStream(#[from] cpal::PlayStreamError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
}

/// Device info shown in the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Name reported by the driver; also used as the selection key.
    pub name: String,
    pub direction: Direction,
    pub is_default: bool,
    pub sample_rate_hz: u32,
    pub channels: u16,
}

/// Lists all input and output devices on the default host (WASAPI on Windows).
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

/// Finds a device by name; with `None`, returns the default device for the direction.
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
