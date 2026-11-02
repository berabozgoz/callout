//! End to end: server process + clients; room create/join, SFU forwarding, speaking event.

use bytes::Bytes;
use callout_net::cert::Invite;
use callout_net::client::Client;
use callout_net::identity::DeviceKey;
use callout_proto::{AudioPacket, ErrorCode, ServerMsg};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

struct Server {
    child: Child,
    invite: Invite,
    dir: std::path::PathBuf,
}

impl Server {
    fn start(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("callout-e2e-{tag}-{}", std::process::id()));
        let port = 40000 + ((std::process::id() as usize + tag.len() * 97) % 20000) as u16;
        let mut child = Command::new(env!("CARGO_BIN_EXE_callout-server"))
            .env("CALLOUT_PORT", port.to_string())
            .env("CALLOUT_DATA", &dir)
            .env("CALLOUT_PUBLIC_IP", "127.0.0.1")
            .env("RUST_LOG", "info")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to start server");
        let stdout = child.stdout.take().unwrap();
        let invite = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(i) = line.find("callout://") {
                    return line[i..].trim().to_string();
                }
            }
            panic!("invite link not seen");
        })
        .join()
        .unwrap();
        Self {
            child,
            invite: Invite::parse(&invite).unwrap(),
            dir,
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Waits for the next message, skipping `Speaking` events (the sender also receives
/// its own speaking event; this test checks them only on B's side).
async fn recv_skip_speaking(c: &mut Client) -> ServerMsg {
    loop {
        match recv(c).await {
            ServerMsg::Speaking { .. } => continue,
            other => return other,
        }
    }
}

async fn recv(c: &mut Client) -> ServerMsg {
    tokio::time::timeout(Duration::from_secs(3), c.control().recv())
        .await
        .expect("no message from server")
        .unwrap()
}

#[tokio::test]
async fn rooms_sfu_and_speaking() {
    let server = Server::start("rooms");
    let invite = &server.invite;

    let mut a = Client::connect(invite, &DeviceKey::generate(), "alice")
        .await
        .expect("A");
    let mut b = Client::connect(invite, &DeviceKey::generate(), "bob")
        .await
        .expect("B");
    let mut c = Client::connect(invite, &DeviceKey::generate(), "carol")
        .await
        .expect("C");

    // A creates a room and gets the code.
    a.create_room("team").await.unwrap();
    let code = match recv(&mut a).await {
        ServerMsg::RoomState { room, members } => {
            assert_eq!(room.name, "team");
            assert_eq!(room.owner, a.user_id());
            assert_eq!(members.len(), 1);
            assert_eq!(room.code.len(), 8);
            room.code
        }
        other => panic!("unexpected: {other:?}"),
    };

    // C tries to join with a wrong code.
    c.join_room("ZZZZZZZZ").await.unwrap();
    match recv(&mut c).await {
        ServerMsg::Error { code, .. } => assert_eq!(code, ErrorCode::RoomNotFound),
        other => panic!("unexpected: {other:?}"),
    }

    // B gets in even when typing lowercase with a dash.
    let typed = format!("{}-{}", &code[..4], &code[4..]).to_lowercase();
    b.join_room(&typed).await.unwrap();
    match recv(&mut b).await {
        ServerMsg::RoomState { room, members } => {
            assert_eq!(room.code, code);
            assert_eq!(members.len(), 2);
        }
        other => panic!("unexpected: {other:?}"),
    }
    match recv(&mut a).await {
        ServerMsg::MemberJoined(m) => assert_eq!(m.user_id, b.user_id()),
        other => panic!("unexpected: {other:?}"),
    }

    // A cannot create a second room while in one.
    a.create_room("x").await.unwrap();
    match recv(&mut a).await {
        ServerMsg::Error { code, .. } => assert_eq!(code, ErrorCode::AlreadyInRoom),
        other => panic!("unexpected: {other:?}"),
    }

    // Audio: A → B, stamped by the server; C (not in the room) does not receive it.
    let opus = [0xAAu8; 40];
    let mut buf = Vec::new();
    AudioPacket {
        sender_id: 0,
        seq: 7,
        timestamp: 960,
        flags: 1,
        opus: &opus,
    }
    .write_to(&mut buf)
    .unwrap();
    let received = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            a.send_audio(Bytes::copy_from_slice(&buf)).unwrap();
            match tokio::time::timeout(Duration::from_millis(200), b.recv_audio()).await {
                Ok(Ok(bytes)) => break bytes,
                _ => continue,
            }
        }
    })
    .await
    .expect("B did not receive an audio packet");
    let p = AudioPacket::parse(&received).unwrap();
    assert_eq!(p.sender_id, a.user_id());
    assert_eq!(p.opus, &opus);
    assert!(
        tokio::time::timeout(Duration::from_millis(300), c.recv_audio())
            .await
            .is_err()
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(300), a.recv_audio())
            .await
            .is_err()
    );

    // Speaking: B gets true, then false after ~300 ms.
    assert_eq!(
        recv(&mut b).await,
        ServerMsg::Speaking {
            user_id: a.user_id(),
            speaking: true
        }
    );
    assert_eq!(
        recv(&mut b).await,
        ServerMsg::Speaking {
            user_id: a.user_id(),
            speaking: false
        }
    );

    // B leaves → A gets MemberLeft; B cannot leave again while not in a room.
    b.leave_room().await.unwrap();
    match recv_skip_speaking(&mut a).await {
        ServerMsg::MemberLeft(id) => assert_eq!(id, b.user_id()),
        other => panic!("unexpected: {other:?}"),
    }
    b.leave_room().await.unwrap();
    match recv(&mut b).await {
        ServerMsg::Error { code, .. } => assert_eq!(code, ErrorCode::NotInRoom),
        other => panic!("unexpected: {other:?}"),
    }

    // Once A leaves too, the room closes: joining with the same code gives RoomNotFound.
    a.leave_room().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    c.join_room(&code).await.unwrap();
    match recv(&mut c).await {
        ServerMsg::Error { code, .. } => assert_eq!(code, ErrorCode::RoomNotFound),
        other => panic!("unexpected: {other:?}"),
    }

    a.close();
    b.close();
    c.close();
}
