//! CallOut sunucusu: SFU + oda yöneticisi + kimlik. Tek binary, tek UDP portu.
//!
//! Faz 0: yalnızca komut satırı ve günlükleme iskeleti. QUIC dinleyici,
//! kendinden imzalı sertifika ve `callout://` davet linki Faz 1 adım 3'te gelir.

use anyhow::Result;
use std::net::SocketAddr;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let port: u16 = std::env::var("CALLOUT_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(callout_net::DEFAULT_PORT);
    let addr: SocketAddr = ([0, 0, 0, 0], port).into();

    tracing::info!(
        %addr,
        protocol = callout_proto::PROTOCOL_VERSION,
        alpn = %String::from_utf8_lossy(callout_net::ALPN),
        "callout-server iskeleti (Faz 0): dinleyici henüz yok"
    );
    Ok(())
}
