//! QUIC taşıma katmanı (quinn).
//!
//! Tek bağlantı, iki yol:
//! - kontrol: çift yönlü bir QUIC akışı, uzunluk önekli `postcard` mesajları
//!   ([`callout_proto::ClientMsg`] / [`callout_proto::ServerMsg`]);
//! - ses: QUIC datagramları ([`callout_proto::AudioPacket`]).
//!
//! Modüller: [`cert`] (sertifika, parmak izi, davet linki, TOFU), [`identity`] (Ed25519
//! cihaz anahtarı), [`control`] (kontrol akışı), [`client`] (bağlantı + el sıkışma).

#![forbid(unsafe_code)]

pub mod cert;
pub mod client;
pub mod control;
pub mod identity;

use callout_proto::ProtoError;
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;

/// QUIC ALPN kimliği; sunucu ve istemci aynı değeri sunmalıdır.
pub const ALPN: &[u8] = b"callout/1";

/// Varsayılan sunucu portu (UDP).
pub const DEFAULT_PORT: u16 = 4433;

/// Kontrol akışında tek mesaj için üst sınır (DoS koruması).
pub const MAX_CONTROL_FRAME: usize = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error(transparent)]
    Proto(#[from] ProtoError),
    #[error("kontrol çerçevesi çok büyük: {0} bayt")]
    FrameTooLarge(usize),
    #[error("eksik veri: {0} bayt daha gerekli")]
    Incomplete(usize),
}

/// İstemci ve sunucunun ortak QUIC taşıma ayarları.
pub fn transport_config() -> quinn::TransportConfig {
    let mut t = quinn::TransportConfig::default();
    t.max_idle_timeout(Some(
        Duration::from_secs(10).try_into().expect("idle timeout"),
    ));
    t.keep_alive_interval(Some(Duration::from_secs(3)));
    // Ses datagramları için tampon: ~1 s @ 16 kişi × 130 bayt × 50/s.
    t.datagram_receive_buffer_size(Some(128 * 1024));
    t.datagram_send_buffer_size(128 * 1024);
    t.max_concurrent_bidi_streams(4u8.into());
    t.max_concurrent_uni_streams(0u8.into());
    t
}

/// `msg`'yi `[u32 LE uzunluk][postcard gövde]` olarak `out`'a ekler.
pub fn frame_control<T: Serialize>(msg: &T, out: &mut Vec<u8>) -> Result<(), NetError> {
    let body = callout_proto::encode(msg)?;
    if body.len() > MAX_CONTROL_FRAME {
        return Err(NetError::FrameTooLarge(body.len()));
    }
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(())
}

/// `buf`'ın başındaki tam bir çerçeveyi çözer; tüketilen bayt sayısını döner.
/// Çerçeve henüz tam değilse [`NetError::Incomplete`] döner ve `buf` değişmez.
pub fn unframe_control<T: DeserializeOwned>(buf: &[u8]) -> Result<(T, usize), NetError> {
    if buf.len() < 4 {
        return Err(NetError::Incomplete(4 - buf.len()));
    }
    let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if len > MAX_CONTROL_FRAME {
        return Err(NetError::FrameTooLarge(len));
    }
    let end = 4 + len;
    if buf.len() < end {
        return Err(NetError::Incomplete(end - buf.len()));
    }
    let msg = callout_proto::decode(&buf[4..end])?;
    Ok((msg, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use callout_proto::ClientMsg;

    #[test]
    fn frame_roundtrip_and_partial() {
        let mut buf = Vec::new();
        frame_control(&ClientMsg::Ping(1), &mut buf).unwrap();
        frame_control(&ClientMsg::SetMuted(true), &mut buf).unwrap();

        let (m1, n1): (ClientMsg, usize) = unframe_control(&buf).unwrap();
        assert_eq!(m1, ClientMsg::Ping(1));
        let (m2, n2): (ClientMsg, usize) = unframe_control(&buf[n1..]).unwrap();
        assert_eq!(m2, ClientMsg::SetMuted(true));
        assert_eq!(n1 + n2, buf.len());

        let r: Result<(ClientMsg, usize), _> = unframe_control(&buf[..n1 - 1]);
        assert!(matches!(r, Err(NetError::Incomplete(1))));
    }
}
