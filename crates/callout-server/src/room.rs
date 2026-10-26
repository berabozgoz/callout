//! Oda durumu ve SFU iletimi.
//!
//! Faz 1 adım 3: tek oda ("lobby"); kimlik doğrulayan herkes otomatik katılır.
//! Oda kodları ve çoklu oda adım 5'te gelir; bu modülün arayüzü o zaman da aynı kalır.
//!
//! İletim kuralı: gelen datagram `stamp_sender` ile damgalanır ve gönderen
//! dışındaki, sağırlaştırılmamış üyelere kopyalanır. Susturulmuş üyenin paketi
//! hiç iletilmez (istemci zaten göndermez; bu sunucu tarafı emniyet).

use bytes::{Bytes, BytesMut};
use callout_proto::{AudioPacket, Member, ServerMsg, UserId, MAX_ROOM_MEMBERS};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// Son ses paketinden bu kadar süre sonra "konuşmuyor" sayılır.
pub const SPEAKING_TIMEOUT: Duration = Duration::from_millis(300);

/// Bir üyenin bağlantı görevine giden kontrol mesajı kuyruğu.
pub type ControlTx = mpsc::UnboundedSender<ServerMsg>;

struct Peer {
    member: Member,
    control: ControlTx,
    conn: quinn::Connection,
    speaking: bool,
    last_audio: Option<Instant>,
}

#[derive(Default)]
pub struct Room {
    peers: HashMap<UserId, Peer>,
}

#[derive(Clone, Default)]
pub struct SharedRoom(Arc<Mutex<Room>>);

impl SharedRoom {
    pub fn new() -> Self {
        Self::default()
    }

    /// Üye ekler; dolu ise `None`. Mevcut üyelere `MemberJoined` yayınlar,
    /// yeni üyeye `RoomState` gönderir.
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
            room: callout_proto::RoomInfo {
                code: "LOBBY".into(),
                name: "Lobi".into(),
                owner: 0,
            },
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

    /// Periyodik (100 ms): süresi dolan konuşmacılar için `Speaking{false}` yayınlar.
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

    /// SFU: `from`'dan gelen ham datagramı damgalayıp diğerlerine iletir.
    /// Kilit yalnızca alıcı listesini kopyalarken tutulur; gönderim kilitsiz.
    pub fn forward_audio(&self, from: UserId, mut packet: BytesMut) {
        if AudioPacket::parse(&packet).is_err() {
            return; // bozuk paket; sessizce düşür
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
            // Datagram gönderimi tıkanmaz; tampon doluysa paket düşer (ses için doğru davranış).
            let _ = conn.send_datagram(packet.clone());
        }
    }
}
