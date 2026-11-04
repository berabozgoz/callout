//! Engine (callout-core) against the server: connect without an audio device, create a room, leave.

use callout_core::{
    Command, ConnectionState, Engine, EngineConfig, ErrorKind, Event, LogUploadOutcome,
};
use std::io::{BufRead, BufReader};
use std::process::{Command as Proc, Stdio};
use std::time::Duration;

async fn next(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>) -> Event {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("no event received")
        .expect("engine shut down")
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
        panic!("no invite link");
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
        panic!("expected Connected")
    };

    engine.send(Command::CreateRoom {
        name: "team".into(),
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
            other => panic!("unexpected: {other:?}"),
        }
    };
    assert_eq!(code.len(), 8);

    // After leaving, the UI should get "not in a room" (the server sends no ack; the engine clears it).
    engine.send(Command::LeaveRoom);
    let mut left = false;
    for _ in 0..6 {
        match next(&mut rx).await {
            Event::RoomUpdated {
                room: None,
                members,
            } => {
                assert!(members.is_empty());
                left = true;
                break;
            }
            _ => continue,
        }
    }
    assert!(left, "RoomUpdated{{room: None}} not received");

    // Code unknown to the server → Error event.
    engine.send(Command::JoinRoom {
        code: "ZZZZZZZZ".into(),
    });
    let mut saw_error = false;
    for _ in 0..6 {
        match next(&mut rx).await {
            Event::Error {
                kind: ErrorKind::RoomNotFound,
                ..
            } => {
                saw_error = true;
                break;
            }
            _ => continue,
        }
    }
    assert!(saw_error, "RoomNotFound not received");

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

/// Starts the server binary; returns the process and the invite link it prints.
fn start_server(port: u16, dir: &std::path::Path) -> (std::process::Child, String) {
    let mut server = Proc::new(env!("CARGO_BIN_EXE_callout-server"))
        .env("CALLOUT_PORT", port.to_string())
        .env("CALLOUT_DATA", dir)
        .env("CALLOUT_PUBLIC_IP", "127.0.0.1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdout = server.stdout.take().unwrap();
    let invite = std::thread::spawn(move || {
        let mut lines = BufReader::new(stdout).lines().map_while(Result::ok);
        let invite = lines
            .by_ref()
            .find_map(|line| {
                line.find("callout://")
                    .map(|i| line[i..].trim().to_string())
            })
            .expect("no invite link");
        // Keep draining stdout so the server never blocks on a full pipe.
        std::thread::spawn(move || lines.for_each(drop));
        invite
    })
    .join()
    .unwrap();
    (server, invite)
}

/// Waits up to `secs` for an event matching `pred`, skipping others.
async fn wait_for(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>,
    secs: u64,
    what: &str,
    pred: impl Fn(&Event) -> bool,
) -> Event {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        let ev = tokio::time::timeout_at(deadline, rx.recv())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
            .expect("engine shut down");
        if pred(&ev) {
            return ev;
        }
    }
}

#[tokio::test]
async fn engine_reconnects_after_server_restart_and_rejoins_its_room() {
    let dir = std::env::temp_dir().join(format!("callout-reconnect-{}", std::process::id()));
    let port = 40000 + ((std::process::id() as usize + 7777) % 20000) as u16;
    let (mut server, invite) = start_server(port, &dir);

    let (engine, mut rx) = Engine::start(EngineConfig {
        data_dir: dir.join("client"),
        no_audio: true,
        ..Default::default()
    });
    engine.send(Command::Connect {
        invite,
        nickname: "test".into(),
    });
    wait_for(&mut rx, 10, "Connected", |e| {
        matches!(e, Event::Connected { .. })
    })
    .await;
    engine.send(Command::CreateRoom {
        name: "team".into(),
    });
    let Event::RoomUpdated {
        room: Some(room), ..
    } = wait_for(&mut rx, 10, "room", |e| {
        matches!(e, Event::RoomUpdated { room: Some(_), .. })
    })
    .await
    else {
        unreachable!()
    };
    let code = room.code;

    // The server dies without closing the connection (crash, reboot, update).
    server.kill().unwrap();
    server.wait().unwrap();
    wait_for(&mut rx, 20, "ConnectionLost", |e| {
        matches!(
            e,
            Event::Error {
                kind: ErrorKind::ConnectionLost,
                ..
            }
        )
    })
    .await;
    wait_for(&mut rx, 5, "Reconnecting", |e| {
        *e == Event::ConnectionState {
            state: ConnectionState::Reconnecting,
        }
    })
    .await;

    // It comes back (same certificate, so the invite still matches): the engine reconnects on its own.
    let (mut server, _) = start_server(port, &dir);
    wait_for(&mut rx, 30, "Connected again", |e| {
        *e == Event::ConnectionState {
            state: ConnectionState::Connected,
        }
    })
    .await;
    // Rooms are persistent: the engine is back in the same room after the restart.
    wait_for(
        &mut rx,
        10,
        "rejoined room",
        |e| matches!(e, Event::RoomUpdated { room: Some(r), .. } if r.code == code),
    )
    .await;

    // Disconnect stops everything; no further reconnect.
    engine.send(Command::Disconnect);
    wait_for(&mut rx, 10, "Disconnected", |e| {
        *e == Event::ConnectionState {
            state: ConnectionState::Disconnected,
        }
    })
    .await;

    drop(engine);
    server.kill().ok();
    server.wait().ok();
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn engine_uploads_a_log_once_per_interval() {
    let dir = std::env::temp_dir().join(format!("callout-logupload-{}", std::process::id()));
    let port = 40000 + ((std::process::id() as usize + 4242) % 20000) as u16;
    let (mut server, invite) = start_server(port, &dir);

    let (engine, mut rx) = Engine::start(EngineConfig {
        data_dir: dir.join("client"),
        no_audio: true,
        ..Default::default()
    });
    // Not connected yet: refused locally.
    engine.send(Command::UploadLog {
        app_version: "test".into(),
        log: "x".into(),
    });
    wait_for(&mut rx, 5, "upload refused while disconnected", |e| {
        matches!(
            e,
            Event::LogUpload {
                outcome: LogUploadOutcome::Failed,
                ..
            }
        )
    })
    .await;

    engine.send(Command::Connect {
        invite,
        nickname: "log tester".into(),
    });
    wait_for(&mut rx, 10, "Connected", |e| {
        *e == Event::ConnectionState {
            state: ConnectionState::Connected,
        }
    })
    .await;

    let log = "2026-11-04T05:00:00Z INFO stats rtt_ms=40\n".repeat(2000);
    engine.send(Command::UploadLog {
        app_version: "0.1.16".into(),
        log: log.clone(),
    });
    let ev = wait_for(&mut rx, 10, "upload result", |e| {
        matches!(e, Event::LogUpload { .. })
    })
    .await;
    assert_eq!(
        ev,
        Event::LogUpload {
            outcome: LogUploadOutcome::Stored,
            detail: String::new()
        }
    );
    // The connection (control stream) is unaffected: a room can still be created.
    engine.send(Command::CreateRoom {
        name: "after upload".into(),
    });
    wait_for(&mut rx, 5, "room", |e| {
        matches!(e, Event::RoomUpdated { room: Some(_), .. })
    })
    .await;

    // A second upload right away is rate limited.
    engine.send(Command::UploadLog {
        app_version: "0.1.16".into(),
        log: "again".into(),
    });
    let ev = wait_for(&mut rx, 10, "second result", |e| {
        matches!(e, Event::LogUpload { .. })
    })
    .await;
    assert!(
        matches!(
            ev,
            Event::LogUpload {
                outcome: LogUploadOutcome::RateLimited,
                ..
            }
        ),
        "{ev:?}"
    );

    let files: Vec<_> = std::fs::read_dir(dir.join("logs"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    assert_eq!(files.len(), 1, "{files:?}");
    let name = files[0].file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.contains("_log_tester_"), "{name}");
    let text = std::fs::read_to_string(&files[0]).unwrap();
    assert!(text.contains("# nickname: log tester\n"), "{text:.200}");
    assert!(text.contains("# app version: 0.1.16\n"));
    assert!(text.ends_with(&log));

    drop(engine);
    server.kill().ok();
    server.wait().ok();
    let _ = std::fs::remove_dir_all(&dir);
}
