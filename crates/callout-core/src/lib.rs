//! İstemci çekirdeği: arayüzden bağımsız durum makinesi.
//!
//! Arayüz (Tauri) yalnızca [`Command`] gönderir ve [`Event`] alır; ses verisi
//! asla bu sınırı geçmez. Faz 0'da yalnızca tipler ve cihaz listesi vardır;
//! oda/oturum mantığı Faz 1 adım 3–5'te gelir.

#![forbid(unsafe_code)]

pub use callout_audio::{list_devices, AudioError, DeviceInfo, Direction};
pub use callout_proto::{Member, RoomInfo, UserId};

/// Arayüz → çekirdek.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Connect { server: String, nickname: String },
    Disconnect,
    CreateRoom { name: String },
    JoinRoom { code: String },
    LeaveRoom,
    SetMuted(bool),
    SetDeafened(bool),
    SetInputDevice(Option<String>),
    SetOutputDevice(Option<String>),
    SetUserVolume { user_id: UserId, gain: f32 },
}

/// Çekirdek → arayüz.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    ConnectionState(ConnectionState),
    RoomUpdated {
        room: Option<RoomInfo>,
        members: Vec<Member>,
    },
    SpeakingChanged {
        user_id: UserId,
        speaking: bool,
    },
    /// Mikrofon seviyesi, 0.0–1.0; arayüzdeki ölçer için ~50 ms'de bir.
    InputLevel(f32),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}
