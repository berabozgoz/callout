//! Client core: a UI-independent state machine.
//!
//! The UI (Tauri or the headless `talk` example) only sends [`Command`]s and
//! receives [`Event`]s; audio data never crosses this boundary. All audio and network work
//! runs inside [`engine::Engine`], on its own threads.

#![forbid(unsafe_code)]

pub mod engine;
mod timing;

use serde::{Deserialize, Serialize};

pub use callout_audio::{list_devices, AudioError, DeviceInfo, Direction};
pub use callout_proto::{Member, RoomInfo, UserId};
pub use engine::{Engine, EngineConfig};

/// UI → core.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    /// Connect with a `callout://` invite link and authenticate.
    Connect {
        invite: String,
        nickname: String,
    },
    Disconnect,
    CreateRoom {
        name: String,
    },
    JoinRoom {
        code: String,
    },
    LeaveRoom,
    SetMuted {
        muted: bool,
    },
    SetDeafened {
        deafened: bool,
    },
    /// Push-to-talk mode: when on, [`Command::PushToTalk`] decides instead of VAD.
    SetPushToTalkMode {
        enabled: bool,
    },
    /// Whether the push-to-talk key is held (meaningful only in push-to-talk mode).
    PushToTalk {
        pressed: bool,
    },
    /// `None` = default device. Restarts the audio pipeline while connected.
    SetInputDevice {
        name: Option<String>,
    },
    SetOutputDevice {
        name: Option<String>,
    },
    /// A user's volume: 0.0 silent, 1.0 normal, 2.0 double.
    SetUserVolume {
        user_id: UserId,
        gain: f32,
    },
    /// Open/close the audio pipeline locally while disconnected (for the level meter and the
    /// sound test in settings). No effect while connected: the pipeline is already open.
    SetAudioPreview {
        enabled: bool,
    },
    /// Microphone test: records 5 s, then plays the recording back the way the room would hear
    /// it (voice detection gate + Opus). No audio goes to the room while recording.
    StartMicTest,
    StopMicTest,
    /// Plays a short test sound on the output device.
    PlayTestSound,
    /// Noise suppression (RNNoise). When toggled, voice detection re-measures the noise floor
    /// (nothing is sent for ~1.5 s).
    SetNoiseSuppression {
        enabled: bool,
    },
    /// Voice activity sensitivity in dB (0 = default; positive picks up quieter speech).
    SetVadSensitivity {
        db: f32,
    },
    /// Automatic gain control: evens out the microphone level before it is sent (quiet mics
    /// are raised, loud speech is lowered). Does not change what voice detection sees.
    SetAgc {
        enabled: bool,
    },
    /// Close the current room for everyone and delete it (only its creator's device may).
    CloseRoom,
    /// Send a diagnostic log to the server's operator (the user agreed in the UI). The app reads
    /// and cleans the log; the engine only carries it. Answered with [`Event::LogUpload`].
    UploadLog {
        app_version: String,
        log: String,
    },
}

/// Core → UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    ConnectionState {
        state: ConnectionState,
    },
    /// Connection established; the ID assigned by the server.
    Connected {
        user_id: UserId,
    },
    /// Room state changed; no `room` means we are not in a room.
    RoomUpdated {
        room: Option<RoomInfo>,
        members: Vec<Member>,
    },
    SpeakingChanged {
        user_id: UserId,
        speaking: bool,
    },
    /// Microphone level, 0.0–1.0; every ~100 ms. `threshold`: the voice detection threshold on
    /// the same scale (audio is sent when the level exceeds it; meaningless in push-to-talk).
    InputLevel {
        level: f32,
        transmitting: bool,
        threshold: f32,
    },
    /// Every 2 s: for the status bar in the UI.
    Stats {
        rtt_ms: u32,
        jitter_ms: u32,
        lost: u32,
        sent: u32,
        received: u32,
    },
    /// The audio pipeline was rebuilt automatically after the audio device was lost.
    /// `fallback_to_default`: the selected device was missing, switched to the default devices.
    AudioRestored {
        fallback_to_default: bool,
    },
    /// Microphone test state. `transmitted_pct`: what percentage of the recording passed as
    /// audio to be sent (the rest counted as silence by voice detection); set once recording ends.
    MicTest {
        state: MicTestState,
        transmitted_pct: Option<u8>,
    },
    /// The audio pipeline (re)started. Input fields are `None` without a microphone (listening
    /// only). A 16 kHz mono input usually means a Bluetooth headset in its hands-free profile.
    AudioStarted {
        input_rate: Option<u32>,
        input_channels: Option<u16>,
    },
    /// `kind` is what the UI translates; `message` is the technical detail (English, for logs
    /// and as a fallback).
    Error {
        kind: ErrorKind,
        message: String,
    },
    /// Result of [`Command::UploadLog`]; `detail` is the technical reason when it failed.
    LogUpload {
        outcome: LogUploadOutcome,
        detail: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogUploadOutcome {
    Stored,
    /// The server does not take logs (an older server, or its operator turned them off).
    Unsupported,
    /// This device sent one a few minutes ago.
    RateLimited,
    /// Not connected, a network error, or the server could not store it.
    Failed,
}

/// Machine-readable error category for [`Event::Error`], so the UI can show it in the user's
/// language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// The invite link could not be parsed.
    InvalidInvite,
    /// The device identity key could not be loaded or created.
    DeviceKey,
    /// Connecting to the server failed (unreachable, wrong fingerprint, rejected).
    ConnectFailed,
    /// The audio devices could not be opened.
    AudioFailed,
    /// Windows denied microphone access (privacy settings; HRESULT 0x80070005).
    MicAccessDenied,
    /// An audio device disappeared; the engine is reopening it.
    AudioDeviceLost,
    /// The connection to the server dropped.
    ConnectionLost,
    /// A command failed (network error while sending, or not valid in the current state).
    RequestFailed,
    // Errors reported by the server (`ServerMsg::Error`).
    VersionMismatch,
    RoomNotFound,
    RoomFull,
    AlreadyInRoom,
    NotInRoom,
    RateLimited,
    ServerError,
    /// Only the room's creator can close it.
    NotOwner,
    /// The room was closed by its owner (shown to every member; not really an error).
    RoomClosed,
}

impl ErrorKind {
    /// Classifies an audio pipeline error message (cpal/WASAPI text).
    pub fn from_audio_error(message: &str) -> Self {
        // E_ACCESSDENIED: Windows microphone privacy setting (or another app holding it exclusively).
        if message.contains("0x80070005") {
            ErrorKind::MicAccessDenied
        } else {
            ErrorKind::AudioFailed
        }
    }

    pub fn from_server(code: callout_proto::ErrorCode) -> Self {
        use callout_proto::ErrorCode as C;
        match code {
            C::VersionMismatch => ErrorKind::VersionMismatch,
            C::RoomNotFound => ErrorKind::RoomNotFound,
            C::RoomFull => ErrorKind::RoomFull,
            C::AlreadyInRoom => ErrorKind::AlreadyInRoom,
            C::NotInRoom => ErrorKind::NotInRoom,
            C::RateLimited => ErrorKind::RateLimited,
            C::NotOwner => ErrorKind::NotOwner,
            C::NotAuthenticated | C::BadSignature | C::Internal => ErrorKind::ServerError,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_mic_access_denied() {
        let msg =
            "input stream: A backend-specific error has occurred: Access is denied. (0x80070005)";
        assert_eq!(ErrorKind::from_audio_error(msg), ErrorKind::MicAccessDenied);
        assert_eq!(
            ErrorKind::from_audio_error("output device: not found"),
            ErrorKind::AudioFailed
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    /// The connection dropped on its own; the engine retries with backoff and rejoins the room.
    /// `Command::Disconnect` stops it.
    Reconnecting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MicTestState {
    /// The audio pipeline just opened and the noise floor is being measured; recording starts next.
    Calibrating,
    Recording,
    Playing,
    Idle,
}
