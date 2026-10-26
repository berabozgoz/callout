//! CallOut sunucusu: SFU + oda + kimlik. Tek binary, tek UDP portu.
//!
//! Çalıştır: `cargo run -p callout-server` → ekrana `callout://IP:port#parmak-izi`
//! davet linki basar. Ortam değişkenleri: `CALLOUT_PORT` (4433), `CALLOUT_DATA`
//! (sertifika klasörü, varsayılan `./callout-data`), `CALLOUT_PUBLIC_IP` (davet
//! linkinde gösterilecek adres; verilmezse yerel adresler listelenir), `RUST_LOG`.

mod room;

use anyhow::{Context, Result};
use bytes::BytesMut;
use callout_net::cert::{Invite, ServerIdentity};
use callout_net::control::ControlStream;
use callout_net::{identity, transport_config, ALPN, DEFAULT_PORT};
use callout_proto::{ClientMsg, ErrorCode, Member, ServerMsg, UserId, PROTOCOL_VERSION};
use room::SharedRoom;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let port: u16 = std::env::var("CALLOUT_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let data_dir =
        PathBuf::from(std::env::var("CALLOUT_DATA").unwrap_or_else(|_| "callout-data".into()));
    let identity = ServerIdentity::load_or_generate(&data_dir).context("sunucu sertifikası")?;
    let fingerprint = identity.fingerprint();

    let mut tls = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_no_client_auth()
    .with_single_cert(vec![identity.cert.clone()], identity.key.clone_key())?;
    tls.alpn_protocols = vec![ALPN.to_vec()];
    let mut cfg = quinn::ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(tls)?,
    ));
    cfg.transport_config(Arc::new(transport_config()));

    let bind: SocketAddr = ([0, 0, 0, 0], port).into();
    let endpoint = quinn::Endpoint::server(cfg, bind).context("UDP portu açılamadı")?;

    tracing::info!(%bind, protocol = PROTOCOL_VERSION, "callout-server dinliyor");
    for ip in advertised_ips() {
        tracing::info!(
            "davet linki: {}",
            Invite::new(SocketAddr::new(ip, port), fingerprint)
        );
    }

    let room = SharedRoom::new();
    let next_id = Arc::new(AtomicU32::new(1));

    // Konuşuyor göstergesi: 100 ms'de bir süresi dolanları kapat.
    let speaking_room = room.clone();
    tokio::spawn(async move {
        let mut t = tokio::time::interval(std::time::Duration::from_millis(100));
        loop {
            t.tick().await;
            speaking_room.tick_speaking();
        }
    });

    while let Some(incoming) = endpoint.accept().await {
        let room = room.clone();
        let next_id = next_id.clone();
        tokio::spawn(async move {
            let remote = incoming.remote_address();
            match incoming.await {
                Ok(conn) => {
                    if let Err(e) = handle_connection(conn, room, next_id).await {
                        tracing::info!(%remote, "bağlantı bitti: {e}");
                    }
                }
                Err(e) => tracing::debug!(%remote, "el sıkışma başarısız: {e}"),
            }
        });
    }
    Ok(())
}

async fn handle_connection(
    conn: quinn::Connection,
    room: SharedRoom,
    next_id: Arc<AtomicU32>,
) -> Result<()> {
    let (send, recv) = conn.accept_bi().await?;
    let mut control: ControlStream<ServerMsg, ClientMsg> = ControlStream::new(send, recv);

    // --- El sıkışma ---
    let (pubkey, nickname) = match control.recv().await? {
        ClientMsg::Hello {
            version,
            device_pubkey,
            nickname,
        } if version == PROTOCOL_VERSION => (device_pubkey, nickname),
        ClientMsg::Hello { version, .. } => {
            control
                .send(&ServerMsg::Error {
                    code: ErrorCode::VersionMismatch,
                    message: format!("protokol {version}, sunucu {PROTOCOL_VERSION}"),
                })
                .await?;
            anyhow::bail!("protokol sürümü uyumsuz: {version}");
        }
        other => anyhow::bail!("Hello bekleniyordu: {other:?}"),
    };
    let user_id: UserId = next_id.fetch_add(1, Ordering::Relaxed);
    let challenge: [u8; 32] = rand::random();
    control
        .send(&ServerMsg::Welcome { user_id, challenge })
        .await?;
    match control.recv().await? {
        ClientMsg::Auth { signature }
            if identity::verify(&pubkey, &challenge, &signature).is_ok() => {}
        _ => {
            control
                .send(&ServerMsg::Error {
                    code: ErrorCode::BadSignature,
                    message: "imza geçersiz".into(),
                })
                .await?;
            anyhow::bail!("imza geçersiz");
        }
    }
    control.send(&ServerMsg::Authed).await?;
    tracing::info!(user_id, nickname, remote = %conn.remote_address(), "kimlik doğrulandı");

    // --- Odaya katıl (Faz 1 adım 3: tek oda) ---
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMsg>();
    let member = Member {
        user_id,
        nickname: nickname.clone(),
        muted: false,
        deafened: false,
    };
    if room.join(member, tx, conn.clone()).is_none() {
        control
            .send(&ServerMsg::Error {
                code: ErrorCode::RoomFull,
                message: "oda dolu".into(),
            })
            .await?;
        anyhow::bail!("oda dolu");
    }

    // --- Ses datagramları: ayrı görev ---
    let audio_conn = conn.clone();
    let audio_room = room.clone();
    let audio_task = tokio::spawn(async move {
        while let Ok(bytes) = audio_conn.read_datagram().await {
            audio_room.forward_audio(user_id, BytesMut::from(&bytes[..]));
        }
    });

    // --- Kontrol döngüsü ---
    let result: Result<()> = async {
        loop {
            tokio::select! {
                msg = control.recv() => match msg? {
                    ClientMsg::Ping(t) => control.send(&ServerMsg::Pong(t)).await?,
                    ClientMsg::SetMuted(m) => room.set_flags(user_id, Some(m), None),
                    ClientMsg::SetDeafened(d) => room.set_flags(user_id, None, Some(d)),
                    ClientMsg::LeaveRoom => break,
                    ClientMsg::CreateRoom { .. } | ClientMsg::JoinRoom { .. } => {
                        control.send(&ServerMsg::Error { code: ErrorCode::Internal, message: "çoklu oda Faz 1 adım 5'te".into() }).await?;
                    }
                    ClientMsg::Hello { .. } | ClientMsg::Auth { .. } => {}
                },
                out = rx.recv() => match out {
                    Some(m) => control.send(&m).await?,
                    None => break,
                },
            }
        }
        Ok(())
    }
    .await;

    audio_task.abort();
    room.leave(user_id);
    tracing::info!(user_id, nickname, members = room.len(), "ayrıldı");
    result
}

/// Davet linkinde gösterilecek adresler: `CALLOUT_PUBLIC_IP` verilmişse o,
/// yoksa makinenin loopback olmayan IPv4 adresleri (yoksa 127.0.0.1).
fn advertised_ips() -> Vec<IpAddr> {
    if let Ok(ip) = std::env::var("CALLOUT_PUBLIC_IP") {
        if let Ok(ip) = ip.parse() {
            return vec![ip];
        }
    }
    // Bağımlılık eklemeden yerel adresi öğrenmenin taşınabilir yolu: bağlanmayan UDP soketi.
    let mut ips = Vec::new();
    if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("8.8.8.8:53").is_ok() {
            if let Ok(local) = sock.local_addr() {
                ips.push(local.ip());
            }
        }
    }
    ips.push(IpAddr::from([127, 0, 0, 1]));
    ips
}
