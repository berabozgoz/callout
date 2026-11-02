//! CallOut server: SFU + rooms + identity. Single binary, single UDP port.
//!
//! Run: `cargo run -p callout-server` → prints a `callout://IP:port#fingerprint`
//! invite link. Environment variables: `CALLOUT_PORT` (4433), `CALLOUT_DATA`
//! (certificate directory, default `./callout-data`), `CALLOUT_PUBLIC_IP` (address to
//! show in the invite link; if unset, local addresses are listed), `RUST_LOG`.

mod room;

use anyhow::{Context, Result};
use bytes::BytesMut;
use callout_net::cert::{Invite, ServerIdentity};
use callout_net::control::ControlStream;
use callout_net::{identity, transport_config, ALPN, DEFAULT_PORT};
use callout_proto::{ClientMsg, ErrorCode, Member, ServerMsg, UserId, PROTOCOL_VERSION};
use room::{Registry, SharedRoom};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};
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
    let identity = ServerIdentity::load_or_generate(&data_dir).context("server certificate")?;
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
    let endpoint = quinn::Endpoint::server(cfg, bind).context("failed to open UDP port")?;

    tracing::info!(%bind, protocol = PROTOCOL_VERSION, "callout-server listening");
    for ip in advertised_ips() {
        tracing::info!(
            "invite link: {}",
            Invite::new(SocketAddr::new(ip, port), fingerprint)
        );
    }

    let registry = Registry::new();
    let next_id = Arc::new(AtomicU32::new(1));

    // Speaking indicator: every 100 ms, clear the ones that have timed out.
    let speaking_reg = registry.clone();
    tokio::spawn(async move {
        let mut t = tokio::time::interval(std::time::Duration::from_millis(100));
        loop {
            t.tick().await;
            speaking_reg.tick_speaking();
        }
    });

    while let Some(incoming) = endpoint.accept().await {
        let registry = registry.clone();
        let next_id = next_id.clone();
        tokio::spawn(async move {
            let remote = incoming.remote_address();
            match incoming.await {
                Ok(conn) => {
                    if let Err(e) = handle_connection(conn, registry, next_id).await {
                        tracing::info!(%remote, "connection ended: {e}");
                    }
                }
                Err(e) => tracing::debug!(%remote, "handshake failed: {e}"),
            }
        });
    }
    Ok(())
}

async fn handle_connection(
    conn: quinn::Connection,
    registry: Registry,
    next_id: Arc<AtomicU32>,
) -> Result<()> {
    let (send, recv) = conn.accept_bi().await?;
    let mut control: ControlStream<ServerMsg, ClientMsg> = ControlStream::new(send, recv);

    // --- Handshake ---
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
                    message: format!("protocol {version}, server {PROTOCOL_VERSION}"),
                })
                .await?;
            anyhow::bail!("protocol version mismatch: {version}");
        }
        other => anyhow::bail!("expected Hello: {other:?}"),
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
                    message: "invalid signature".into(),
                })
                .await?;
            anyhow::bail!("invalid signature");
        }
    }
    control.send(&ServerMsg::Authed).await?;
    tracing::info!(user_id, nickname, remote = %conn.remote_address(), "authenticated");

    // --- Room state: the user starts out in no room ---
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMsg>();
    let member = Member {
        user_id,
        nickname: nickname.clone(),
        muted: false,
        deafened: false,
    };
    let current: Arc<RwLock<Option<SharedRoom>>> = Arc::new(RwLock::new(None));

    // --- Audio datagrams: separate task; forwarded only while in a room ---
    let audio_conn = conn.clone();
    let audio_current = current.clone();
    let audio_task = tokio::spawn(async move {
        while let Ok(bytes) = audio_conn.read_datagram().await {
            let room = audio_current.read().unwrap().clone();
            if let Some(room) = room {
                room.forward_audio(user_id, BytesMut::from(&bytes[..]));
            }
        }
    });

    // --- Stats log: RTT, loss and datagram counts every 10 s (for the gate test) ---
    let stats_conn = conn.clone();
    let stats_current = current.clone();
    let stats_task = tokio::spawn(async move {
        let mut t = tokio::time::interval(std::time::Duration::from_secs(10));
        t.tick().await;
        loop {
            t.tick().await;
            let in_room = stats_current.read().unwrap().is_some();
            let st = stats_conn.stats();
            tracing::info!(
                user_id,
                in_room,
                rtt_ms = st.path.rtt.as_millis() as u64,
                lost_packets = st.path.lost_packets,
                cwnd = st.path.cwnd,
                dgram_rx = st.udp_rx.datagrams,
                dgram_tx = st.udp_tx.datagrams,
                "connection stats"
            );
        }
    });
    let connected_at = std::time::Instant::now();

    // Removes the user from the room; returns the left room's code (`None` if not in a room).
    let leave = |current: &Arc<RwLock<Option<SharedRoom>>>, registry: &Registry| {
        let room = current.write().unwrap().take();
        room.map(|room| {
            let code = room.code();
            room.leave(user_id);
            registry.remove_if_empty(&room);
            code
        })
    };

    // --- Control loop ---
    let result: Result<()> = async {
        loop {
            tokio::select! {
                msg = control.recv() => match msg? {
                    ClientMsg::Ping(t) => control.send(&ServerMsg::Pong(t)).await?,
                    ClientMsg::CreateRoom { name } => {
                        if current.read().unwrap().is_some() {
                            control.send(&ServerMsg::Error { code: ErrorCode::AlreadyInRoom, message: "leave the room first".into() }).await?;
                            continue;
                        }
                        let name = if name.trim().is_empty() { format!("{nickname}'s room") } else { name.trim().chars().take(32).collect() };
                        let room = registry.create(name, user_id);
                        room.join(member.clone(), tx.clone(), conn.clone());
                        tracing::info!(user_id, code = room.code(), "room created");
                        *current.write().unwrap() = Some(room);
                    }
                    ClientMsg::JoinRoom { code } => {
                        if current.read().unwrap().is_some() {
                            control.send(&ServerMsg::Error { code: ErrorCode::AlreadyInRoom, message: "leave the room first".into() }).await?;
                            continue;
                        }
                        match registry.get(&code) {
                            None => control.send(&ServerMsg::Error { code: ErrorCode::RoomNotFound, message: format!("room not found: {code}") }).await?,
                            Some(room) => {
                                if room.join(member.clone(), tx.clone(), conn.clone()).is_none() {
                                    control.send(&ServerMsg::Error { code: ErrorCode::RoomFull, message: "room is full".into() }).await?;
                                } else {
                                    tracing::info!(user_id, code = room.code(), members = room.len(), "joined room");
                                    *current.write().unwrap() = Some(room);
                                }
                            }
                        }
                    }
                    ClientMsg::LeaveRoom => {
                        if let Some(code) = leave(&current, &registry) {
                            tracing::info!(user_id, code, "left room");
                        } else {
                            control.send(&ServerMsg::Error { code: ErrorCode::NotInRoom, message: "you are not in a room".into() }).await?;
                        }
                    }
                    ClientMsg::SetMuted(m) => {
                        tracing::info!(user_id, muted = m, "mic state");
                        if let Some(r) = current.read().unwrap().as_ref() { r.set_flags(user_id, Some(m), None); }
                    }
                    ClientMsg::SetDeafened(d) => {
                        tracing::info!(user_id, deafened = d, "headphone state");
                        if let Some(r) = current.read().unwrap().as_ref() { r.set_flags(user_id, None, Some(d)); }
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
    stats_task.abort();
    if let Some(code) = leave(&current, &registry) {
        tracing::info!(user_id, code, "removed from room on disconnect");
    }
    let st = conn.stats();
    tracing::info!(
        user_id,
        nickname,
        rooms = registry.len(),
        secs = connected_at.elapsed().as_secs(),
        rtt_ms = st.path.rtt.as_millis() as u64,
        lost_packets = st.path.lost_packets,
        close_reason = ?conn.close_reason(),
        "connection closed"
    );
    result
}

/// Addresses to show in the invite link: `CALLOUT_PUBLIC_IP` if set,
/// otherwise the machine's non-loopback IPv4 addresses (or 127.0.0.1 if none).
fn advertised_ips() -> Vec<IpAddr> {
    if let Ok(ip) = std::env::var("CALLOUT_PUBLIC_IP") {
        if let Ok(ip) = ip.parse() {
            return vec![ip];
        }
    }
    // Portable way to learn the local address without adding a dependency: a UDP socket that never sends.
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
