//! Room state and SFU forwarding.
//!
//! [`Registry`] maps a room code to its room. Rooms are persistent: an empty room keeps its code
//! (friends can always rejoin with the same code, and reconnecting clients find their room after a
//! server restart). The list is saved to `rooms.tsv` in the data directory; rooms unused for
//! `ROOM_TTL` are dropped. Codes are 8 characters long and avoid confusable characters (I, O, 0, 1).
//!
//! Forwarding rule: an incoming datagram is stamped with `stamp_sender` and copied
//! to every non-deafened member except the sender. A muted member's packets are
//! never forwarded (the client does not send them anyway; this is a server-side safeguard).

use bytes::{Bytes, BytesMut};
use callout_proto::{
    AudioPacket, Member, RoomInfo, ServerMsg, UserId, MAX_ROOM_MEMBERS, ROOM_CODE_LEN,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

/// A member counts as "not speaking" this long after their last audio packet.
pub const SPEAKING_TIMEOUT: Duration = Duration::from_millis(300);

/// Queue of control messages to a member's connection task.
pub type ControlTx = mpsc::UnboundedSender<ServerMsg>;

struct Peer {
    member: Member,
    /// The member's device public key (ownership check).
    key: [u8; 32],
    control: ControlTx,
    conn: quinn::Connection,
    speaking: bool,
    last_audio: Option<Instant>,
}

pub struct Room {
    info: RoomInfo,
    peers: HashMap<UserId, Peer>,
    /// Device key of the creator; `None` for rooms loaded from an older room file.
    owner_key: Option<[u8; 32]>,
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

fn parse_key(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut key = [0u8; 32];
    for (i, b) in key.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(key)
}

/// An empty room is forgotten after this long without anyone in it.
pub const ROOM_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);
/// Upper bound on stored rooms; creating more drops the longest-unused empty room.
const MAX_ROOMS: usize = 1000;

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Default)]
struct Rooms {
    rooms: HashMap<String, SharedRoom>,
    /// When each room was last in use (unix seconds): creation, or when the last member left.
    last_used: HashMap<String, u64>,
    /// `rooms.tsv` (`code<TAB>last_used<TAB>owner key hex or -<TAB>name` per line); `None` keeps
    /// rooms in memory only.
    path: Option<PathBuf>,
}

impl Rooms {
    fn save(&self) {
        let Some(path) = &self.path else { return };
        let mut out = String::new();
        for (code, room) in &self.rooms {
            let name: String = room.name().chars().filter(|c| !c.is_control()).collect();
            let used = self.last_used.get(code).copied().unwrap_or_else(unix_now);
            let owner = room
                .owner_key()
                .map(|k| k.iter().map(|b| format!("{b:02x}")).collect::<String>())
                .unwrap_or_else(|| "-".into());
            out.push_str(&format!("{code}\t{used}\t{owner}\t{name}\n"));
        }
        // Write-then-rename so a crash never leaves a half-written file.
        let tmp = path.with_extension("tsv.tmp");
        if let Err(e) = std::fs::write(&tmp, out).and_then(|_| std::fs::rename(&tmp, path)) {
            tracing::warn!("could not save rooms: {e}");
        }
    }

    /// Drops empty rooms unused for longer than `ttl`; returns how many.
    fn expire(&mut self, now: u64, ttl: Duration) -> usize {
        let stale: Vec<String> = self
            .rooms
            .iter()
            .filter(|(code, room)| {
                room.len() == 0
                    && now.saturating_sub(self.last_used.get(*code).copied().unwrap_or(now))
                        > ttl.as_secs()
            })
            .map(|(code, _)| code.clone())
            .collect();
        for code in &stale {
            self.rooms.remove(code);
            self.last_used.remove(code);
            tracing::info!(code, "room expired");
        }
        stale.len()
    }
}

/// Code → room registry.
#[derive(Clone, Default)]
pub struct Registry(Arc<Mutex<Rooms>>);

impl Registry {
    /// Registry backed by `path`: loads the rooms saved there (skipping expired ones) and saves
    /// every change. A missing or unreadable file starts empty.
    pub fn load(path: PathBuf) -> Self {
        let mut rooms = Rooms {
            path: Some(path.clone()),
            ..Default::default()
        };
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for line in text.lines() {
            let mut parts = line.splitn(4, '\t');
            let (Some(code), Some(used), Some(owner), Some(name)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            let owner_key = parse_key(owner);
            let Ok(used) = used.parse::<u64>() else {
                continue;
            };
            let code = normalize_code(code);
            if code.len() != ROOM_CODE_LEN {
                continue;
            }
            // Owners are per-connection ids; they mean nothing after a restart.
            let room = SharedRoom::new(
                RoomInfo {
                    code: code.clone(),
                    name: name.to_string(),
                    owner: 0,
                },
                owner_key,
            );
            rooms.rooms.insert(code.clone(), room);
            rooms.last_used.insert(code, used);
        }
        let expired = rooms.expire(unix_now(), ROOM_TTL);
        if expired > 0 {
            rooms.save();
        }
        tracing::info!(rooms = rooms.rooms.len(), expired, "rooms loaded");
        Self(Arc::new(Mutex::new(rooms)))
    }

    /// Creates and registers a new room; the code is guaranteed unique.
    pub fn create(&self, name: String, owner: UserId, owner_key: [u8; 32]) -> SharedRoom {
        let mut map = self.0.lock().unwrap();
        if map.rooms.len() >= MAX_ROOMS {
            // Make room by dropping the longest-unused empty room.
            let oldest = map
                .rooms
                .iter()
                .filter(|(_, r)| r.len() == 0)
                .min_by_key(|(code, _)| map.last_used.get(*code).copied().unwrap_or(0))
                .map(|(code, _)| code.clone());
            if let Some(code) = oldest {
                map.rooms.remove(&code);
                map.last_used.remove(&code);
            }
        }
        let code = loop {
            let c = generate_code();
            if !map.rooms.contains_key(&c) {
                break c;
            }
        };
        let room = SharedRoom::new(
            RoomInfo {
                code: code.clone(),
                name,
                owner,
            },
            Some(owner_key),
        );
        map.rooms.insert(code.clone(), room.clone());
        map.last_used.insert(code, unix_now());
        map.save();
        room
    }

    pub fn get(&self, code: &str) -> Option<SharedRoom> {
        self.0
            .lock()
            .unwrap()
            .rooms
            .get(&normalize_code(code))
            .cloned()
    }

    /// Called after a member left: an empty room is kept (same code later) and its last use noted.
    pub fn member_left(&self, room: &SharedRoom) {
        if room.len() == 0 {
            let code = room.code();
            let mut map = self.0.lock().unwrap();
            map.last_used.insert(code.clone(), unix_now());
            map.save();
            tracing::info!(code, "room empty (kept)");
        }
    }

    /// Removes a room for good (owner closed it); members are not touched here.
    pub fn delete(&self, room: &SharedRoom) {
        let code = room.code();
        let mut map = self.0.lock().unwrap();
        map.rooms.remove(&code);
        map.last_used.remove(&code);
        map.save();
    }

    /// Drops rooms unused for longer than [`ROOM_TTL`]; call periodically.
    pub fn expire(&self) {
        let mut map = self.0.lock().unwrap();
        if map.expire(unix_now(), ROOM_TTL) > 0 {
            map.save();
        }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.0.lock().unwrap().rooms.len()
    }

    /// Clears timed-out speakers in all rooms.
    pub fn tick_speaking(&self) {
        let rooms: Vec<SharedRoom> = self.0.lock().unwrap().rooms.values().cloned().collect();
        for r in rooms {
            r.tick_speaking();
        }
    }
}

impl SharedRoom {
    pub fn new(info: RoomInfo, owner_key: Option<[u8; 32]>) -> Self {
        Self(Arc::new(Mutex::new(Room {
            info,
            peers: HashMap::new(),
            owner_key,
        })))
    }

    pub fn owner_key(&self) -> Option<[u8; 32]> {
        self.0.lock().unwrap().owner_key
    }

    pub fn is_owner(&self, key: &[u8; 32]) -> bool {
        self.0.lock().unwrap().owner_key.as_ref() == Some(key)
    }

    /// Closes the room for everyone: each member gets `RoomClosed` and is removed.
    pub fn close(&self) {
        let mut room = self.0.lock().unwrap();
        let msg = ServerMsg::RoomClosed {
            code: room.info.code.clone(),
        };
        for p in room.peers.values() {
            let _ = p.control.send(msg.clone());
        }
        room.peers.clear();
    }

    pub fn name(&self) -> String {
        self.0.lock().unwrap().info.name.clone()
    }

    pub fn code(&self) -> String {
        self.0.lock().unwrap().info.code.clone()
    }

    /// Adds a member; `None` if the room is full. Broadcasts `MemberJoined` to existing
    /// members and sends `RoomState` to the new member.
    pub fn join(
        &self,
        member: Member,
        key: [u8; 32],
        control: ControlTx,
        conn: quinn::Connection,
    ) -> Option<()> {
        let mut room = self.0.lock().unwrap();
        if room.peers.len() >= MAX_ROOM_MEMBERS {
            return None;
        }
        // Tell the joiner who owns the room in this session's ids (0 if the owner is away).
        let owner_key = room.owner_key;
        room.info.owner = if owner_key == Some(key) {
            member.user_id
        } else {
            room.peers
                .values()
                .find(|p| Some(p.key) == owner_key)
                .map(|p| p.member.user_id)
                .unwrap_or(0)
        };
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
                key,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("callout-rooms-{tag}-{}.tsv", std::process::id()))
    }

    #[test]
    fn rooms_survive_a_restart_and_keep_their_code() {
        let path = temp_file("restart");
        let _ = std::fs::remove_file(&path);
        let reg = Registry::load(path.clone());
        let room = reg.create("team\twith tab".into(), 7, [9; 32]);
        let code = room.code();
        reg.member_left(&room); // empty: kept
        assert!(reg.get(&code).is_some());

        let reloaded = Registry::load(path.clone());
        let again = reloaded.get(&code).expect("room lost on restart");
        assert_eq!(again.name(), "teamwith tab");
        assert!(again.is_owner(&[9; 32]), "owner lost on restart");
        assert!(!again.is_owner(&[8; 32]));
        assert_eq!(reloaded.len(), 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unused_rooms_expire() {
        let path = temp_file("expire");
        let old = unix_now() - ROOM_TTL.as_secs() - 10;
        let fresh = unix_now() - 60;
        std::fs::write(
            &path,
            format!("AAAAAAAA\t{old}\t-\told\nBBBBBBBB\t{fresh}\t-\tfresh\nbad line\n"),
        )
        .unwrap();
        let reg = Registry::load(path.clone());
        assert!(reg.get("AAAA-AAAA").is_none(), "expired room kept");
        assert!(reg.get("bbbb-bbbb").is_some());
        // The expired room is gone from the file too.
        assert!(!std::fs::read_to_string(&path).unwrap().contains("AAAAAAAA"));
        let _ = std::fs::remove_file(&path);
    }
}
