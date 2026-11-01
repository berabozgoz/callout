//! İstemci çekirdeği: arayüzden bağımsız durum makinesi.
//!
//! Arayüz (Tauri ya da headless `talk` örneği) yalnızca [`Command`] gönderir ve
//! [`Event`] alır; ses verisi asla bu sınırı geçmez. Tüm ses ve ağ işi
//! [`engine::Engine`] içinde, kendi iş parçacıklarında döner.

#![forbid(unsafe_code)]

pub mod engine;

use serde::{Deserialize, Serialize};

pub use callout_audio::{list_devices, AudioError, DeviceInfo, Direction};
pub use callout_proto::{Member, RoomInfo, UserId};
pub use engine::{Engine, EngineConfig};

/// Arayüz → çekirdek.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    /// `callout://` davet linkiyle bağlan ve kimlik doğrula.
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
    /// Bas-konuş modu: açıkken VAD yerine [`Command::PushToTalk`] belirler.
    SetPushToTalkMode {
        enabled: bool,
    },
    /// Bas-konuş tuşu basılı mı (yalnızca bas-konuş modunda anlamlı).
    PushToTalk {
        pressed: bool,
    },
    /// `None` = varsayılan cihaz. Bağlıyken ses hattını yeniden başlatır.
    SetInputDevice {
        name: Option<String>,
    },
    SetOutputDevice {
        name: Option<String>,
    },
    /// Bir kullanıcının sesi: 0.0 sessiz, 1.0 normal, 2.0 iki kat.
    SetUserVolume {
        user_id: UserId,
        gain: f32,
    },
    /// Bağlı değilken ses hattını yerel olarak aç/kapat (ayarlardaki seviye çubuğu ve ses
    /// testi için). Bağlıyken etkisiz: hat zaten açık.
    SetAudioPreview {
        enabled: bool,
    },
    /// Mikrofon testi: 5 sn kayıt, ardından kayıt odadakilerin duyacağı haliyle (konuşma
    /// algılama kapısı + Opus) geri çalınır. Kayıt sırasında odaya ses gitmez.
    StartMicTest,
    StopMicTest,
    /// Çıkış cihazında kısa bir test sesi çalar.
    PlayTestSound,
    /// Gürültü bastırma (RNNoise). Değişince konuşma algılama gürültü tabanını yeniden ölçer
    /// (~1.5 sn gönderim yok).
    SetNoiseSuppression {
        enabled: bool,
    },
}

/// Çekirdek → arayüz.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    ConnectionState {
        state: ConnectionState,
    },
    /// Bağlantı kuruldu; sunucunun verdiği kimlik.
    Connected {
        user_id: UserId,
    },
    /// Oda durumu değişti; `room` yoksa odada değiliz.
    RoomUpdated {
        room: Option<RoomInfo>,
        members: Vec<Member>,
    },
    SpeakingChanged {
        user_id: UserId,
        speaking: bool,
    },
    /// Mikrofon seviyesi, 0.0–1.0; ~100 ms'de bir. `threshold`: konuşma algılama eşiği,
    /// aynı ölçekte (seviye bunu geçince ses gönderilir; bas-konuşta anlamsız).
    InputLevel {
        level: f32,
        transmitting: bool,
        threshold: f32,
    },
    /// Her 2 s: arayüzdeki durum çubuğu için.
    Stats {
        rtt_ms: u32,
        jitter_ms: u32,
        lost: u32,
        sent: u32,
        received: u32,
    },
    /// Ses cihazı kaybolduktan sonra ses hattı otomatik yeniden kuruldu.
    /// `fallback_to_default`: seçili cihaz yoktu, varsayılan cihazlara geçildi.
    AudioRestored {
        fallback_to_default: bool,
    },
    /// Mikrofon testi durumu. `transmitted_pct`: kaydın yüzde kaçı gönderilecek ses olarak
    /// geçti (gerisi konuşma algılamada sessizlik sayıldı); kayıt bitince dolu.
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
    /// Ses hattı yeni açıldı, gürültü tabanı ölçülüyor; kayıt ardından başlar.
    Calibrating,
    Recording,
    Playing,
    Idle,
}
