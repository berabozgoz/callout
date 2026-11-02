//! Room state and SFU forwarding.
//!
//! [`Registry`] maps a room code to its room; a room is removed once it becomes empty.
//! Codes are 8 characters long and avoid confusable characters (I, O, 0, 1).
//!
//! Forwarding rule: an incoming datagram is stamped with `stamp_sender` and copied
//! to every non-deafened member except the sender. A muted member's packets are
//! never forwarded (the client does not send them anyway; this is a server-side safeguard).

use bytes::{Bytes, BytesMut};
use callout_proto::{
    AudioPacket, Member, RoomInfo, ServerMsg, UserId, MAX_ROOM_MEMBERS, ROOM_CODE_LEN,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// A member counts as "not speaking" this long after their last audio packet.
pub const SPEAKING_TIMEOUT: Duration = Duration::from_millis(300);

/// Queue of control messages to a member's connection task.
pub type ControlTx = mpsc::UnboundedSender<ServerMsg>;

struct Peer {
    member: Member,
    control: ControlTx,
    conn: quinn::Connection,
    speaking: bool,
    last_audio: Option<Instant>,
}

pub struct Room {
    info: RoomInfo,
    peers: HashMap<UserId, Peer>,
}

#[derive(Clone)]
pub struct SharedRoom(Arc<Mutex<Room>>);

/// Room code alphabet: no I, O, 0, 1.
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// Generates a random room code (e.g. `K7Q2MX9P`).
pub fn generate_code() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..ROOM_CODE_LEN)
        .map(|_| CODE_ALPHABET[rng.gen_range(0..CODE_ALPHABET.len())] as char)
        .collect()
}

/// Normalizes a user-typed code: uppercased, dashes/spaces stripped.
pub fn normalize_code(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Code → room registry.
#[derive(Clone, Default)]
pub struct Registry(Arc<Mutex<HashMap<String, SharedRoom>>>);

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates and registers a new room; the code is guaranteed unique.
    pub fn create(&self, name: String, owner: UserId) -> SharedRoom {
        let mut map = self.0.lock().unwrap();
        let code = loop {
            let c = generate_code();
            if !map.contains_key(&c) {
                break c;
            }
        };
        let room = SharedRoom::new(RoomInfo {
            code: code.clone(),
            name,
            owner,
        });
        map.insert(code, room.clone());
        room
    }

    pub fn get(&self, code: &str) -> Option<SharedRoom> {
        self.0.lock().unwrap().get(&normalize_code(code)).cloned()
    }

    /// Removes the room from the registry if it is empty.
    pub fn remove_if_empty(&self, room: &SharedRoom) {
        let code = room.code();
        let mut map = self.0.lock().unwrap();
        if room.len() == 0 {
            map.remove(&code);
            tracing::info!(code, "room closed");
        }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.0.lock().unwrap().len()
    }

    /// Clears timed-out speakers in all rooms.
    pub fn tick_speaking(&self) {
        let rooms: Vec<SharedRoom> = self.0.lock().unwrap().values().cloned().collect();
        for r in rooms {
            r.tick_speaking();
        }
    }
}

impl SharedRoom {
    pub fn new(info: RoomInfo) -> Self {
        Self(Arc::new(Mutex::new(Room {
            info,
            peers: HashMap::new(),
        })))
    }

    pub fn code(&self) -> String {
        self.0.lock().unwrap().info.code.clone()
    }

    /// Adds a member; `None` if the room is full. Broadcasts `MemberJoined` to existing
    /// members and sends `RoomState` to the new member.
    pub fn join(&self, member: Member, control: ControlTx, conn: quinn::Connection) -> Option<()> {
        let mut room = self.0.lock().unwrap();
        if room.peers.len() >= MAX_ROOM_MEMBERS {
            return None;
        }
        for p in room.peers.values() {
            let _ = p.control.send(ServerMsg::MemberJoined(member.clone()));
        }
        let members: Vec<Member> = room
            .peers
            .values()
            .map(|p| p.member.clone())
            .chain(std::iter::once(member.clone()))
            .collect();
        let _ = control.send(ServerMsg::RoomState {
            room: room.info.clone(),
            members,
        });
        room.peers.insert(
            member.user_id,
            Peer {
                member,
                control,
                conn,
                speaking: false,
                last_audio: None,
            },
        );
        Some(())
    }

    pub fn leave(&self, user_id: UserId) {
        let mut room = self.0.lock().unwrap();
        if room.peers.remove(&user_id).is_some() {
            for p in room.peers.values() {
                let _ = p.control.send(ServerMsg::MemberLeft(user_id));
            }
        }
    }

    pub fn set_flags(&self, user_id: UserId, muted: Option<bool>, deafened: Option<bool>) {
        let mut room = self.0.lock().unwrap();
        let Some(p) = room.peers.get_mut(&user_id) else {
            return;
        };
        if let Some(m) = muted {
            p.member.muted = m;
        }
        if let Some(d) = deafened {
            p.member.deafened = d;
        }
        let msg = ServerMsg::MemberUpdated {
            user_id,
            muted: p.member.muted,
            deafened: p.member.deafened,
        };
        for p in room.peers.values() {
            let _ = p.control.send(msg.clone());
        }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.0.lock().unwrap().peers.len()
    }

    /// Periodic (100 ms): broadcasts `Speaking{false}` for timed-out speakers.
    pub fn tick_speaking(&self) {
        let mut room = self.0.lock().unwrap();
        let now = Instant::now();
        let ended: Vec<UserId> = room
            .peers
            .iter()
            .filter(|(_, p)| p.speaking && p.last_audio.is_some_and(|t| now - t > SPEAKING_TIMEOUT))
            .map(|(id, _)| *id)
            .collect();
        for id in ended {
            if let Some(p) = room.peers.get_mut(&id) {
                p.speaking = false;
            }
            let msg = ServerMsg::Speaking {
                user_id: id,
                speaking: false,
            };
            for p in room.peers.values() {
                let _ = p.control.send(msg.clone());
            }
        }
    }

    /// SFU: stamps the raw datagram from `from` and forwards it to the others.
    /// The lock is held only while copying the recipient list; sending is lock-free.
    pub fn forward_audio(&self, from: UserId, mut packet: BytesMut) {
        if AudioPacket::parse(&packet).is_err() {
            return; // malformed packet; drop silently
        }
        AudioPacket::stamp_sender(&mut packet, from);
        let packet: Bytes = packet.freeze();

        let targets: Vec<quinn::Connection> = {
            let mut room = self.0.lock().unwrap();
            let Some(sender) = room.peers.get_mut(&from) else {
                return;
            };
            if sender.member.muted {
                return;
            }
            sender.last_audio = Some(Instant::now());
            if !sender.speaking {
                sender.speaking = true;
                let msg = ServerMsg::Speaking {
                    user_id: from,
                    speaking: true,
                };
                for p in room.peers.values() {
                    let _ = p.control.send(msg.clone());
                }
            }
            room.peers
                .iter()
                .filter(|(id, p)| **id != from && !p.member.deafened)
                .map(|(_, p)| p.conn.clone())
                .collect()
        };
        for conn in targets {
            // Sending a datagram never blocks; if the buffer is full the packet is dropped (the right behavior for audio).
            let _ = conn.send_datagram(packet.clone());
        }
    }
}
