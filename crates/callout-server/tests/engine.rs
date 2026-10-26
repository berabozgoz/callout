//! Motor (callout-core) ile sunucu: ses cihazı olmadan bağlan, oda aç, ayrıl.

use callout_core::{Command, ConnectionState, Engine, EngineConfig, Event};
use std::io::{BufRead, BufReader};
use std::process::{Command as Proc, Stdio};
use std::time::Duration;

async fn next(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>) -> Event {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("olay gelmedi")
        .expect("motor kapandı")
}

#[tokio::test]
async fn engine_connects_creates_room_and_disconnects() {
    let dir = std::env::temp_dir().join(format!("callout-engine-{}", std::process::id()));
    let port = 40000 + ((std::process::id() as usize + 1234) % 20000) as u16;
    let mut server = Proc::new(env!("CARGO_BIN_EXE_callout-server"))
        .env("CALLOUT_PORT", port.to_string())
        .env("CALLOUT_DATA", &dir)
        .env("CALLOUT_PUBLIC_IP", "127.0.0.1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdout = server.stdout.take().unwrap();
    let invite = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(i) = line.find("callout://") {
                return line[i..].trim().to_string();
            }
        }
        panic!("davet linki yok");
    })
    .join()
    .unwrap();

    let (engine, mut rx) = Engine::start(EngineConfig {
        data_dir: dir.join("client"),
        no_audio: true,
        ..Default::default()
    });
    assert_eq!(
        next(&mut rx).await,
        Event::ConnectionState {
            state: ConnectionState::Disconnected
        }
    );

    engine.send(Command::Connect {
        invite,
        nickname: "test".into(),
    });
    assert_eq!(
        next(&mut rx).await,
        Event::ConnectionState {
            state: ConnectionState::Connecting
        }
    );
    assert_eq!(
        next(&mut rx).await,
        Event::ConnectionState {
            state: ConnectionState::Connected
        }
    );
    let Event::Connected { user_id } = next(&mut rx).await else {
        panic!("Connected bekleniyordu")
    };

    engine.send(Command::CreateRoom {
        name: "takım".into(),
    });
    let code = loop {
        match next(&mut rx).await {
            Event::RoomUpdated {
                room: Some(r),
                members,
            } => {
                assert_eq!(members.len(), 1);
                assert_eq!(members[0].user_id, user_id);
                break r.code;
            }
            Event::Stats { .. } => continue,
            other => panic!("beklenmeyen: {other:?}"),
        }
    };
    assert_eq!(code.len(), 8);

    // Sunucuda bilinmeyen kod → Error olayı.
    engine.send(Command::LeaveRoom);
    engine.send(Command::JoinRoom {
        code: "ZZZZZZZZ".into(),
    });
    let mut saw_error = false;
    for _ in 0..6 {
        match next(&mut rx).await {
            Event::Error { message } if message.contains("RoomNotFound") => {
                saw_error = true;
                break;
            }
            _ => continue,
        }
    }
    assert!(saw_error, "RoomNotFound gelmedi");

    engine.send(Command::Disconnect);
    let mut disconnected = false;
    for _ in 0..6 {
        if let Event::ConnectionState {
            state: ConnectionState::Disconnected,
        } = next(&mut rx).await
        {
            disconnected = true;
            break;
        }
    }
    assert!(disconnected);

    drop(engine);
    server.kill().ok();
    server.wait().ok();
    let _ = std::fs::remove_dir_all(&dir);
}
