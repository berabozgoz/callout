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
    Error {
        message: String,
    },
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
