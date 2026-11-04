//! CallOut protocol: types shared by the client and the server.
//!
//! There are two separate channels:
//! - **Control stream** (reliable QUIC stream): [`ClientMsg`] / [`ServerMsg`],
//!   serialized with `postcard`, each message prefixed with a 4-byte LE length.
//! - **Audio datagram** (unreliable QUIC datagram): [`AudioPacket`], a fixed
//!   header + Opus payload. The header uses a hand-written fixed layout rather
//!   than `postcard` so the server can overwrite `sender_id` without copying.
//!
//! Rules:
//! - An incompatible change bumps [`PROTOCOL_VERSION`]; the server rejects old
//!   clients with [`ServerMsg::Error`] + [`ErrorCode::VersionMismatch`].
//! - When adding something, append it to the end of the enum variants; the order
//!   determines `postcard`'s wire format.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

/// Control protocol version. Bump on incompatible changes.
pub const PROTOCOL_VERSION: u16 = 1;

/// Room code length (uppercase letters + digits, e.g. `K7Q2-MX9P`).
pub const ROOM_CODE_LEN: usize = 8;

/// Maximum members per room in the MVP (target: a team of 5–10).
pub const MAX_ROOM_MEMBERS: usize = 16;

/// Audio clock: 48 kHz. One 20 ms frame = 960 samples.
pub const SAMPLE_RATE_HZ: u32 = 48_000;
pub const FRAME_MS: u32 = 20;
pub const FRAME_SAMPLES: u32 = SAMPLE_RATE_HZ * FRAME_MS / 1000;

/// Per-session ID the server assigns to a user.
pub type UserId = u32;

/// Ed25519 public key.
pub type PublicKey = [u8; 32];
/// Ed25519 signature.
pub type Signature = [u8; 64];

// ---------------------------------------------------------------------------
// Control messages
// ---------------------------------------------------------------------------

/// Client → server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientMsg {
    /// First message on the connection. The signature cannot be verified before
    /// the server sends the `challenge` in `Welcome`, so the flow is:
    /// `Hello` → `Welcome{challenge}` → `Auth{signature}` → `Authed`.
    Hello {
        version: u16,
        device_pubkey: PublicKey,
        nickname: String,
    },
    /// Ed25519 signature over `Welcome.challenge`.
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
    /// RTT measurement; the server echoes the same value in `Pong`.
    Ping(u64),
    /// Close the current room for everyone and delete it from the server. Only the device that
    /// created the room may do this (`ErrorCode::NotOwner` otherwise).
    CloseRoom,
}

/// Server → client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerMsg {
    Welcome {
        user_id: UserId,
        challenge: [u8; 32],
    },
    /// Authenticated; room commands are now accepted.
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
    /// The room was closed by its owner; every member is now outside any room.
    RoomClosed {
        code: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomInfo {
    pub code: String,
    pub name: String,
    /// Session id of the room's creator if they are in the room when this is sent, else 0.
    /// Ownership itself belongs to the creator's device key, so it survives reconnects and restarts.
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
    // Appended after `Internal`: serde/postcard encode the variant index, not the number.
    NotOwner = 9,
}

// ---------------------------------------------------------------------------
// Diagnostic log upload (a stream of its own)
// ---------------------------------------------------------------------------

/// Upper limit of an uploaded log (the client sends the end of its log file).
pub const MAX_LOG_UPLOAD: usize = 1024 * 1024;

/// Header of a log upload. A user who agrees sends the app's diagnostic log to the server's
/// operator on a **second bidirectional stream**, opened after authentication (the control stream
/// stays the first one): one length-prefixed [`LogUpload`], then `bytes` bytes of log text, then
/// the client finishes the stream; the server answers with one length-prefixed
/// [`LogUploadReply`]. The control messages are untouched: a server without upload support never
/// accepts the stream (the client gives up after a timeout) and older clients never open one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogUpload {
    pub app_version: String,
    pub bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogUploadReply {
    Stored,
    /// More than [`MAX_LOG_UPLOAD`], or fewer bytes than announced.
    Rejected,
    /// This device uploaded a log a short while ago.
    RateLimited,
    /// The operator turned uploads off.
    Disabled,
    /// The server could not store it (disk full, I/O error).
    Failed,
}

// ---------------------------------------------------------------------------
// Framing and serialization (control stream)
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ProtoError {
    #[error("serialization error: {0}")]
    Codec(#[from] postcard::Error),
    #[error("audio packet too short: {0} bytes")]
    PacketTooShort(usize),
    #[error("audio payload too large: {0} bytes (max {MAX_OPUS_PAYLOAD})")]
    PayloadTooLarge(usize),
}

/// Serializes a control message to bytes with `postcard` (without the length prefix).
pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, ProtoError> {
    Ok(postcard::to_allocvec(msg)?)
}

/// Reads a control message from bytes.
pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, ProtoError> {
    Ok(postcard::from_bytes(bytes)?)
}

// ---------------------------------------------------------------------------
// Audio packet (datagram)
// ---------------------------------------------------------------------------

/// Audio packet header length: sender_id(4) + seq(2) + timestamp(4) + flags(1).
pub const AUDIO_HEADER_LEN: usize = 11;

/// A 20 ms Opus frame is ~80 bytes at 32 kbps; headroom for FEC.
pub const MAX_OPUS_PAYLOAD: usize = 400;

/// `flags` bits.
pub mod flags {
    /// Talk start (first packet after silence); the jitter buffer may be reset.
    pub const TALK_START: u8 = 0b0000_0001;
    /// Payload contains Opus in-band FEC.
    pub const HAS_FEC: u8 = 0b0000_0010;
    /// Payload is end-to-end encrypted with the room key (later phase; always 0 in the MVP).
    pub const E2E: u8 = 0b0000_0100;
}

/// A single 20 ms audio packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioPacket<'a> {
    /// 0 when the client sends; the server fills in the sender's ID when forwarding.
    pub sender_id: UserId,
    /// Sequence number, +1 per packet, wraps on overflow.
    pub seq: u16,
    /// 48 kHz sample clock; consecutive packets differ by [`FRAME_SAMPLES`].
    pub timestamp: u32,
    pub flags: u8,
    pub opus: &'a [u8],
}

impl<'a> AudioPacket<'a> {
    /// Writes header + payload to `out`; returns the total length written.
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

    /// Parses a packet from a raw datagram; the payload is not copied.
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

    /// Server side: overwrites the `sender_id` field without copying the packet.
    /// Must only be called on a buffer already validated by `parse`.
    pub fn stamp_sender(bytes: &mut [u8], sender_id: UserId) {
        debug_assert!(bytes.len() >= AUDIO_HEADER_LEN);
        bytes[..4].copy_from_slice(&sender_id.to_le_bytes());
    }
}

// ---------------------------------------------------------------------------
// Tests
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
                name: "team".into(),
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
    fn log_upload_roundtrip() {
        let h = LogUpload {
            app_version: "0.1.16".into(),
            bytes: 1234,
        };
        assert_eq!(decode::<LogUpload>(&encode(&h).unwrap()).unwrap(), h);
        for r in [
            LogUploadReply::Stored,
            LogUploadReply::Rejected,
            LogUploadReply::RateLimited,
            LogUploadReply::Disabled,
            LogUploadReply::Failed,
        ] {
            assert_eq!(decode::<LogUploadReply>(&encode(&r).unwrap()).unwrap(), r);
        }
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
