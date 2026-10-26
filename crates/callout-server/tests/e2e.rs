//! Uçtan uca: sunucu süreci + iki istemci; A'nın ses datagramı B'ye sunucu damgasıyla ulaşır.

use bytes::Bytes;
use callout_net::cert::Invite;
use callout_net::client::Client;
use callout_net::identity::DeviceKey;
use callout_proto::{AudioPacket, ServerMsg};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;

#[tokio::test]
async fn two_clients_exchange_audio_through_sfu() {
    let dir = std::env::temp_dir().join(format!("callout-e2e-{}", std::process::id()));
    let port = 40000 + (std::process::id() % 20000) as u16;
    let mut server = Command::new(env!("CARGO_BIN_EXE_callout-server"))
        .env("CALLOUT_PORT", port.to_string())
        .env("CALLOUT_DATA", &dir)
        .env("CALLOUT_PUBLIC_IP", "127.0.0.1")
        .env("RUST_LOG", "info")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("sunucu başlatılamadı");

    // Davet linkini stdout'taki günlükten yakala (tracing_subscriber::fmt varsayılanı stdout).
    let stdout = server.stdout.take().unwrap();
    let invite = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(i) = line.find("callout://") {
                return line[i..].trim().to_string();
            }
        }
        panic!("davet linki görülmedi");
    })
    .join()
    .unwrap();
    let invite = Invite::parse(&invite).unwrap();

    let ka = DeviceKey::generate();
    let kb = DeviceKey::generate();
    let mut a = Client::connect(&invite, &ka, "ayse")
        .await
        .expect("A bağlanamadı");
    let mut b = Client::connect(&invite, &kb, "bora")
        .await
        .expect("B bağlanamadı");
    assert_ne!(a.user_id(), b.user_id());

    // A: RoomState (tek üye) sonra MemberJoined(B)
    match a.control().recv().await.unwrap() {
        ServerMsg::RoomState { members, .. } => assert_eq!(members.len(), 1),
        other => panic!("beklenmeyen: {other:?}"),
    }
    match a.control().recv().await.unwrap() {
        ServerMsg::MemberJoined(m) => assert_eq!(m.user_id, b.user_id()),
        other => panic!("beklenmeyen: {other:?}"),
    }
    // B: RoomState (iki üye)
    match b.control().recv().await.unwrap() {
        ServerMsg::RoomState { members, .. } => assert_eq!(members.len(), 2),
        other => panic!("beklenmeyen: {other:?}"),
    }

    // A ses gönderir; datagram kaybı olabilir, birkaç kez dene.
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
    .expect("B ses paketi almadı");

    let p = AudioPacket::parse(&received).unwrap();
    assert_eq!(p.sender_id, a.user_id(), "sunucu sender_id damgalamalı");
    assert_eq!(p.seq, 7);
    assert_eq!(p.opus, &opus);

    // Konuşuyor göstergesi: B'ye Speaking{true}, ~300 ms sonra Speaking{false} gelir.
    let first = tokio::time::timeout(Duration::from_secs(2), b.control().recv())
        .await
        .expect("Speaking{true} gelmedi")
        .unwrap();
    assert_eq!(
        first,
        ServerMsg::Speaking {
            user_id: a.user_id(),
            speaking: true
        }
    );
    let second = tokio::time::timeout(Duration::from_secs(2), b.control().recv())
        .await
        .expect("Speaking{false} gelmedi")
        .unwrap();
    assert_eq!(
        second,
        ServerMsg::Speaking {
            user_id: a.user_id(),
            speaking: false
        }
    );

    // A kendi paketini geri almamalı.
    assert!(
        tokio::time::timeout(Duration::from_millis(300), a.recv_audio())
            .await
            .is_err()
    );

    a.close();
    b.close();
    server.kill().ok();
    server.wait().ok();
    let _ = std::fs::remove_dir_all(&dir);
}
