//! Client core: a UI-independent state machine.
//!
//! The UI (Tauri or the headless `talk` example) only sends [`Command`]s and
//! receives [`Event`]s; audio data never crosses this boundary. All audio and network work
//! runs inside [`engine::Engine`], on its own threads.

#![forbid(unsafe_code)]

pub mod engine;

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
    /// `kind` is what the UI translates; `message` is the technical detail (English, for logs
    /// and as a fallback).
    Error {
        kind: ErrorKind,
        message: String,
    },
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
