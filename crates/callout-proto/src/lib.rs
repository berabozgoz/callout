//! CallOut protokolü: istemci ve sunucunun paylaştığı tipler.
//!
//! İki ayrı kanal vardır:
//! - **Kontrol akışı** (güvenilir QUIC akışı): [`ClientMsg`] / [`ServerMsg`],
//!   `postcard` ile serileştirilir, her mesajın önünde 4 baytlık LE uzunluk gelir.
//! - **Ses datagramı** (güvenilmez QUIC datagramı): [`AudioPacket`], sabit
//!   başlık + Opus yükü. Başlık `postcard` değil, elle yazılan sabit düzen
//!   kullanır ki sunucu `sender_id`'yi kopyalama yapmadan üstüne yazabilsin.
//!
//! Kurallar:
//! - Uyumsuz bir değişiklik [`PROTOCOL_VERSION`]'ı artırır; sunucu eski istemciyi
//!   [`ServerMsg::Error`] + [`ErrorCode::VersionMismatch`] ile reddeder.
//! - Yeni alan eklerken enum varyantlarının sonuna ekleyin; sıralama
//!   `postcard`'ın tel biçimini belirler.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

/// Kontrol protokolü sürümü. Uyumsuz değişiklikte artırın.
pub const PROTOCOL_VERSION: u16 = 1;

/// Oda kodu uzunluğu (büyük harf + rakam, ör. `K7Q2-MX9P`).
pub const ROOM_CODE_LEN: usize = 8;

/// MVP'de bir odadaki en fazla üye sayısı (hedef: 5–10 kişilik takım).
pub const MAX_ROOM_MEMBERS: usize = 16;

/// Ses saati: 48 kHz. Bir 20 ms çerçeve = 960 örnek.
pub const SAMPLE_RATE_HZ: u32 = 48_000;
pub const FRAME_MS: u32 = 20;
pub const FRAME_SAMPLES: u32 = SAMPLE_RATE_HZ * FRAME_MS / 1000;

/// Sunucunun bir kullanıcıya verdiği oturum içi kimlik.
pub type UserId = u32;

/// Ed25519 açık anahtarı.
pub type PublicKey = [u8; 32];
/// Ed25519 imzası.
pub type Signature = [u8; 64];

// ---------------------------------------------------------------------------
// Kontrol mesajları
// ---------------------------------------------------------------------------

/// İstemci → sunucu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientMsg {
    /// Bağlantıdaki ilk mesaj. Sunucu `Welcome` içindeki `challenge`'ı
    /// göndermeden önce imza doğrulanamaz; bu yüzden akış şöyledir:
    /// `Hello` → `Welcome{challenge}` → `Auth{signature}` → `Authed`.
    Hello {
        version: u16,
        device_pubkey: PublicKey,
        nickname: String,
    },
    /// `Welcome.challenge` üzerine Ed25519 imzası.
    Auth {
        #[serde(with = "BigArray")]
        signature: Signature,
    },
    CreateRoom {
        name: String,
    },
    JoinRoom {
        code: String,
    },
    LeaveRoom,
    SetMuted(bool),
    SetDeafened(bool),
    /// RTT ölçümü; sunucu aynı değeri `Pong` ile döner.
    Ping(u64),
}

/// Sunucu → istemci.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerMsg {
    Welcome {
        user_id: UserId,
        challenge: [u8; 32],
    },
    /// Kimlik doğrulandı; artık oda komutları kabul edilir.
    Authed,
    RoomState {
        room: RoomInfo,
        members: Vec<Member>,
    },
    MemberJoined(Member),
    MemberLeft(UserId),
    MemberUpdated {
        user_id: UserId,
        muted: bool,
        deafened: bool,
    },
    Speaking {
        user_id: UserId,
        speaking: bool,
    },
    Error {
        code: ErrorCode,
        message: String,
    },
    Pong(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomInfo {
    pub code: String,
    pub name: String,
    pub owner: UserId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub user_id: UserId,
    pub nickname: String,
    pub muted: bool,
    pub deafened: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum ErrorCode {
    VersionMismatch = 1,
    NotAuthenticated = 2,
    BadSignature = 3,
    RoomNotFound = 4,
    RoomFull = 5,
    AlreadyInRoom = 6,
    NotInRoom = 7,
    RateLimited = 8,
    Internal = 100,
}

// ---------------------------------------------------------------------------
// Çerçeveleme ve serileştirme (kontrol akışı)
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ProtoError {
    #[error("serileştirme hatası: {0}")]
    Codec(#[from] postcard::Error),
    #[error("ses paketi çok kısa: {0} bayt")]
    PacketTooShort(usize),
    #[error("ses yükü çok büyük: {0} bayt (en fazla {MAX_OPUS_PAYLOAD})")]
    PayloadTooLarge(usize),
}

/// Kontrol mesajını `postcard` ile baytlara çevirir (uzunluk öneki olmadan).
pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, ProtoError> {
    Ok(postcard::to_allocvec(msg)?)
}

/// Baytlardan kontrol mesajını okur.
pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, ProtoError> {
    Ok(postcard::from_bytes(bytes)?)
}

// ---------------------------------------------------------------------------
// Ses paketi (datagram)
// ---------------------------------------------------------------------------

/// Ses paketi başlık uzunluğu: sender_id(4) + seq(2) + timestamp(4) + flags(1).
pub const AUDIO_HEADER_LEN: usize = 11;

/// Opus 20 ms çerçevesi 32 kbps'de ~80 bayt; FEC ile sınır payı.
pub const MAX_OPUS_PAYLOAD: usize = 400;

/// `flags` bitleri.
pub mod flags {
    /// Konuşma başlangıcı (sessizlikten sonra ilk paket); jitter tamponu sıfırlanabilir.
    pub const TALK_START: u8 = 0b0000_0001;
    /// Yük Opus in-band FEC içeriyor.
    pub const HAS_FEC: u8 = 0b0000_0010;
    /// Yük oda anahtarıyla uçtan uca şifreli (ileri faz; MVP'de hep 0).
    pub const E2E: u8 = 0b0000_0100;
}

/// Tek bir 20 ms'lik ses paketi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioPacket<'a> {
    /// İstemci gönderirken 0; sunucu iletirken gönderenin kimliğiyle doldurur.
    pub sender_id: UserId,
    /// Sıra numarası, her pakette +1, taşmada sarar.
    pub seq: u16,
    /// 48 kHz örnek saati; ardışık paketler arasında [`FRAME_SAMPLES`] fark.
    pub timestamp: u32,
    pub flags: u8,
    pub opus: &'a [u8],
}

impl<'a> AudioPacket<'a> {
    /// Başlık + yükü `out`'a yazar; yazılan toplam uzunluğu döner.
    pub fn write_to(&self, out: &mut Vec<u8>) -> Result<usize, ProtoError> {
        if self.opus.len() > MAX_OPUS_PAYLOAD {
            return Err(ProtoError::PayloadTooLarge(self.opus.len()));
        }
        out.clear();
        out.reserve(AUDIO_HEADER_LEN + self.opus.len());
        out.extend_from_slice(&self.sender_id.to_le_bytes());
        out.extend_from_slice(&self.seq.to_le_bytes());
        out.extend_from_slice(&self.timestamp.to_le_bytes());
        out.push(self.flags);
        out.extend_from_slice(self.opus);
        Ok(out.len())
    }

    /// Ham datagramdan paketi ayrıştırır; yük kopyalanmaz.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, ProtoError> {
        if bytes.len() < AUDIO_HEADER_LEN {
            return Err(ProtoError::PacketTooShort(bytes.len()));
        }
        let opus = &bytes[AUDIO_HEADER_LEN..];
        if opus.len() > MAX_OPUS_PAYLOAD {
            return Err(ProtoError::PayloadTooLarge(opus.len()));
        }
        Ok(Self {
            sender_id: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            seq: u16::from_le_bytes([bytes[4], bytes[5]]),
            timestamp: u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]),
            flags: bytes[10],
            opus,
        })
    }

    /// Sunucu tarafı: paketi kopyalamadan `sender_id` alanının üstüne yazar.
    /// Yalnızca `parse` ile doğrulanmış bir tamponda çağrılmalıdır.
    pub fn stamp_sender(bytes: &mut [u8], sender_id: UserId) {
        debug_assert!(bytes.len() >= AUDIO_HEADER_LEN);
        bytes[..4].copy_from_slice(&sender_id.to_le_bytes());
    }
}

// ---------------------------------------------------------------------------
// Testler
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn control_roundtrip() {
        let msgs = vec![
            ClientMsg::Hello {
                version: PROTOCOL_VERSION,
                device_pubkey: [7; 32],
                nickname: "bera".into(),
            },
            ClientMsg::Auth { signature: [9; 64] },
            ClientMsg::JoinRoom {
                code: "K7Q2MX9P".into(),
            },
            ClientMsg::SetMuted(true),
            ClientMsg::Ping(42),
        ];
        for m in msgs {
            let bytes = encode(&m).unwrap();
            let back: ClientMsg = decode(&bytes).unwrap();
            assert_eq!(m, back);
        }

        let s = ServerMsg::RoomState {
            room: RoomInfo {
                code: "K7Q2MX9P".into(),
                name: "takım".into(),
                owner: 1,
            },
            members: vec![Member {
                user_id: 1,
                nickname: "bera".into(),
                muted: false,
                deafened: false,
            }],
        };
        let bytes = encode(&s).unwrap();
        let back: ServerMsg = decode(&bytes).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn error_code_wire_size_is_small() {
        let m = ServerMsg::Error {
            code: ErrorCode::RoomFull,
            message: String::new(),
        };
        assert!(encode(&m).unwrap().len() <= 4);
    }

    #[test]
    fn audio_packet_roundtrip_and_stamp() {
        let opus = [1u8, 2, 3, 4, 5];
        let p = AudioPacket {
            sender_id: 0,
            seq: 65535,
            timestamp: 960 * 3,
            flags: flags::TALK_START,
            opus: &opus,
        };
        let mut buf = Vec::new();
        let n = p.write_to(&mut buf).unwrap();
        assert_eq!(n, AUDIO_HEADER_LEN + opus.len());

        AudioPacket::stamp_sender(&mut buf, 77);
        let back = AudioPacket::parse(&buf).unwrap();
        assert_eq!(back.sender_id, 77);
        assert_eq!(back.seq, 65535);
        assert_eq!(back.timestamp, 2880);
        assert_eq!(back.flags, flags::TALK_START);
        assert_eq!(back.opus, &opus);
    }

    #[test]
    fn audio_packet_rejects_short_and_oversized() {
        assert!(matches!(
            AudioPacket::parse(&[0; 5]),
            Err(ProtoError::PacketTooShort(5))
        ));
        let big = vec![0u8; MAX_OPUS_PAYLOAD + 1];
        let p = AudioPacket {
            sender_id: 0,
            seq: 0,
            timestamp: 0,
            flags: 0,
            opus: &big,
        };
        assert!(matches!(
            p.write_to(&mut Vec::new()),
            Err(ProtoError::PayloadTooLarge(_))
        ));
    }

    proptest! {
        #[test]
        fn audio_header_roundtrip(sender in any::<u32>(), seq in any::<u16>(), ts in any::<u32>(), fl in any::<u8>(), opus in proptest::collection::vec(any::<u8>(), 0..=MAX_OPUS_PAYLOAD)) {
            let p = AudioPacket { sender_id: sender, seq, timestamp: ts, flags: fl, opus: &opus };
            let mut buf = Vec::new();
            p.write_to(&mut buf).unwrap();
            let back = AudioPacket::parse(&buf).unwrap();
            prop_assert_eq!(back, p);
        }

        #[test]
        fn client_msg_roundtrip(nick in "[a-zA-Z0-9 ]{0,32}", muted in any::<bool>()) {
            for m in [ClientMsg::Hello { version: 1, device_pubkey: [0; 32], nickname: nick.clone() }, ClientMsg::SetMuted(muted)] {
                let bytes = encode(&m).unwrap();
                let back: ClientMsg = decode(&bytes).unwrap();
                prop_assert_eq!(back, m);
            }
        }
    }
}
