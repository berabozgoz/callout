//! Headless konuşma istemcisi: [`callout_core::Engine`] üzerine ince bir sarmalayıcı.
//! Tauri uygulamasıyla aynı motoru kullanır; terminalden test ve ölçüm içindir.
//!
//! Çalıştır:
//!   cargo run -p callout-core --example talk -- "callout://IP:4433#PARMAK_IZI" TakmaAd --create [--name OdaAdı]
//!   cargo run -p callout-core --example talk -- "callout://IP:4433#PARMAK_IZI" TakmaAd --room KOD
//! Ek: [--ptt] (bas-konuş: Enter'a basınca 3 sn gönderir) [--volume ID=KAZANÇ] [--input AD] [--output AD]

use anyhow::{Context, Result};
use callout_core::{Command, Engine, EngineConfig, Event};
use std::path::PathBuf;
use std::time::Duration;

enum RoomArg {
    Create(String),
    Join(String),
}

struct Args {
    invite: String,
    nickname: String,
    room: RoomArg,
    volumes: Vec<(u32, f32)>,
    ptt: bool,
    input: Option<String>,
    output: Option<String>,
}

fn parse_args() -> Result<Args> {
    let mut it = std::env::args().skip(1);
    let invite = it.next().context("1. argüman: callout:// davet linki")?;
    let nickname = it.next().context("2. argüman: takma ad")?;
    let mut room = None;
    let mut volumes = Vec::new();
    let mut ptt = false;
    let (mut input, mut output) = (None, None);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--create" => room = Some(RoomArg::Create(String::new())),
            "--name" => {
                if let Some(RoomArg::Create(n)) = room.as_mut() {
                    *n = it.next().context("--name OdaAdı")?;
                } else {
                    anyhow::bail!("--name yalnızca --create ile kullanılır");
                }
            }
            "--room" => room = Some(RoomArg::Join(it.next().context("--room KOD")?)),
            "--volume" => {
                let v = it.next().context("--volume ID=KAZANÇ")?;
                let (id, g) = v.split_once('=').context("--volume ID=KAZANÇ")?;
                volumes.push((id.parse()?, g.parse()?));
            }
            "--ptt" => ptt = true,
            "--input" => input = Some(it.next().context("--input AD")?),
            "--output" => output = Some(it.next().context("--output AD")?),
            other => anyhow::bail!("bilinmeyen argüman: {other}"),
        }
    }
    let room = room.context("--create ya da --room KOD gerekli")?;
    Ok(Args {
        invite,
        nickname,
        room,
        volumes,
        ptt,
        input,
        output,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();
    let args = parse_args()?;

    let (engine, mut events) = Engine::start(EngineConfig {
        data_dir: PathBuf::from("callout-data"),
        input_device: args.input.clone(),
        output_device: args.output.clone(),
        push_to_talk: args.ptt,
        no_audio: false,
    });
    engine.send(Command::Connect {
        invite: args.invite.clone(),
        nickname: args.nickname.clone(),
    });
    for (user_id, gain) in &args.volumes {
        engine.send(Command::SetUserVolume {
            user_id: *user_id,
            gain: *gain,
        });
    }

    // Bas-konuş: stdin'de Enter → 3 sn gönder.
    if args.ptt {
        let tx = engine_clone_sender(&engine);
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            let mut line = String::new();
            while stdin.read_line(&mut line).is_ok() {
                tx(Command::PushToTalk { pressed: true });
                std::thread::sleep(Duration::from_secs(3));
                tx(Command::PushToTalk { pressed: false });
                line.clear();
            }
        });
        tracing::info!("bas-konuş modu: Enter'a basınca 3 sn gönderilir");
    }

    let mut joined = false;
    let mut last_level_log = std::time::Instant::now();
    loop {
        tokio::select! {
            ev = events.recv() => {
                let Some(ev) = ev else { break };
                match ev {
                    Event::ConnectionState { state } => tracing::info!(?state, "bağlantı"),
                    Event::Connected { user_id } => {
                        tracing::info!(user_id, "bağlandı");
                        match &args.room {
                            RoomArg::Create(name) => engine.send(Command::CreateRoom { name: name.clone() }),
                            RoomArg::Join(code) => engine.send(Command::JoinRoom { code: code.clone() }),
                        }
                    }
                    Event::RoomUpdated { room, members } => {
                        let names: Vec<_> = members.iter().map(|m| format!("{}#{}{}", m.nickname, m.user_id, if m.muted { " (sessiz)" } else { "" })).collect();
                        match room {
                            Some(r) => {
                                if !joined {
                                    joined = true;
                                    tracing::info!("ODA KODU: {}-{}  (arkadaşlarına ver: --room {})", &r.code[..4], &r.code[4..], r.code);
                                }
                                tracing::info!(name = r.name, ?names, "oda");
                            }
                            None => {
                                tracing::info!("odadan çıkıldı");
                                break;
                            }
                        }
                    }
                    Event::SpeakingChanged { user_id, speaking } => tracing::info!(user_id, speaking, "konuşuyor"),
                    Event::InputLevel { level, transmitting } => {
                        if last_level_log.elapsed() >= Duration::from_secs(5) {
                            last_level_log = std::time::Instant::now();
                            tracing::info!(level = format!("{level:.2}"), transmitting, "mikrofon");
                        }
                    }
                    Event::Stats { rtt_ms, jitter_ms, lost, sent, received } => {
                        tracing::info!(rtt_ms, jitter_ms, lost, sent, received, "istatistik (2 sn)");
                    }
                    Event::AudioRestored { fallback_to_default } => {
                        tracing::info!(fallback_to_default, "ses hattı kurtarıldı");
                    }
                    Event::Error { message } => {
                        tracing::error!(message, "hata");
                        if message.contains("RoomNotFound") || message.contains("RoomFull") || message.starts_with("bağlanılamadı") || message.starts_with("bağlantı koptu") {
                            break;
                        }
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("çıkılıyor");
                break;
            }
        }
    }
    engine.send(Command::Disconnect);
    tokio::time::sleep(Duration::from_millis(200)).await;
    Ok(())
}

/// `Engine` `Clone` değildir; PTT iş parçacığı için gönderici kapatması.
fn engine_clone_sender(engine: &Engine) -> impl Fn(Command) + Send + 'static {
    let tx = engine.sender();
    move |c| {
        let _ = tx.send(c);
    }
}
