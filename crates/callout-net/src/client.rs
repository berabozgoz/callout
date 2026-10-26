//! İstemci tarafı QUIC bağlantısı ve el sıkışma.
//!
//! Akış: `Hello` → `Welcome{challenge}` → `Auth{signature}` → `Authed`.
//! Sonrasında [`Client::control`] üzerinden oda mesajları, [`Client::send_audio`] /
//! [`Client::recv_audio`] üzerinden ses datagramları.

use crate::cert::{Invite, PinnedVerifier};
use crate::control::{ControlError, ControlStream};
use crate::identity::DeviceKey;
use crate::{transport_config, ALPN};
use bytes::Bytes;
use callout_proto::{ClientMsg, ErrorCode, ServerMsg, UserId, PROTOCOL_VERSION};
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("yerel UDP soketi: {0}")]
    Socket(#[from] std::io::Error),
    #[error("TLS yapılandırması: {0}")]
    Tls(#[from] rustls::Error),
    #[error("QUIC yapılandırması: {0}")]
    QuicConfig(#[from] quinn::crypto::rustls::NoInitialCipherSuite),
    #[error("bağlanılamadı: {0}")]
    Connect(#[from] quinn::ConnectError),
    #[error("bağlantı koptu: {0}")]
    Connection(#[from] quinn::ConnectionError),
    #[error(transparent)]
    Control(#[from] ControlError),
    #[error("sunucu reddetti: {code:?}: {message}")]
    Rejected { code: ErrorCode, message: String },
    #[error("el sıkışmada beklenmeyen mesaj: {0:?}")]
    Unexpected(ServerMsg),
    #[error("datagram gönderilemedi: {0}")]
    Datagram(#[from] quinn::SendDatagramError),
}

pub struct Client {
    conn: quinn::Connection,
    control: ControlStream<ClientMsg, ServerMsg>,
    user_id: UserId,
}

impl Client {
    /// Davet linkindeki sunucuya bağlanır ve kimlik doğrulamasını tamamlar.
    pub async fn connect(
        invite: &Invite,
        key: &DeviceKey,
        nickname: &str,
    ) -> Result<Self, ClientError> {
        let bind: SocketAddr = if invite.addr.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        }
        .parse()
        .unwrap();
        let mut endpoint = quinn::Endpoint::client(bind)?;

        let mut tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(PinnedVerifier::new(invite.fingerprint))
        .with_no_client_auth();
        tls.alpn_protocols = vec![ALPN.to_vec()];
        let mut cfg = quinn::ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(tls)?,
        ));
        cfg.transport_config(Arc::new(transport_config()));
        endpoint.set_default_client_config(cfg);

        let conn = endpoint.connect(invite.addr, "callout")?.await?;
        let (send, recv) = conn.open_bi().await?;
        let mut control = ControlStream::new(send, recv);

        control
            .send(&ClientMsg::Hello {
                version: PROTOCOL_VERSION,
                device_pubkey: key.public_key(),
                nickname: nickname.to_string(),
            })
            .await?;
        let (user_id, challenge) = match control.recv().await? {
            ServerMsg::Welcome { user_id, challenge } => (user_id, challenge),
            ServerMsg::Error { code, message } => {
                return Err(ClientError::Rejected { code, message })
            }
            other => return Err(ClientError::Unexpected(other)),
        };
        control
            .send(&ClientMsg::Auth {
                signature: key.sign(&challenge),
            })
            .await?;
        match control.recv().await? {
            ServerMsg::Authed => {}
            ServerMsg::Error { code, message } => {
                return Err(ClientError::Rejected { code, message })
            }
            other => return Err(ClientError::Unexpected(other)),
        }

        tracing::info!(user_id, rtt_ms = conn.rtt().as_millis(), "bağlandı");
        Ok(Self {
            conn,
            control,
            user_id,
        })
    }

    pub fn user_id(&self) -> UserId {
        self.user_id
    }

    pub fn control(&mut self) -> &mut ControlStream<ClientMsg, ServerMsg> {
        &mut self.control
    }

    /// Oda açar; yanıt kontrol akışından `RoomState` ya da `Error` olarak gelir.
    pub async fn create_room(&mut self, name: &str) -> Result<(), ClientError> {
        Ok(self
            .control
            .send(&ClientMsg::CreateRoom {
                name: name.to_string(),
            })
            .await?)
    }

    /// Koda göre odaya girer; yanıt `RoomState` ya da `Error`.
    pub async fn join_room(&mut self, code: &str) -> Result<(), ClientError> {
        Ok(self
            .control
            .send(&ClientMsg::JoinRoom {
                code: code.to_string(),
            })
            .await?)
    }

    pub async fn leave_room(&mut self) -> Result<(), ClientError> {
        Ok(self.control.send(&ClientMsg::LeaveRoom).await?)
    }

    pub fn rtt(&self) -> std::time::Duration {
        self.conn.rtt()
    }

    /// Hazır bir ses paketini (başlık + Opus) güvenilmez datagram olarak gönderir.
    pub fn send_audio(&self, packet: Bytes) -> Result<(), ClientError> {
        Ok(self.conn.send_datagram(packet)?)
    }

    /// Bir sonraki ses datagramını bekler.
    pub async fn recv_audio(&self) -> Result<Bytes, ClientError> {
        Ok(self.conn.read_datagram().await?)
    }

    /// Datagram alımı için bağımsız bir tutamaç (ayrı görevde kullanmak için).
    pub fn connection(&self) -> quinn::Connection {
        self.conn.clone()
    }

    pub fn close(&self) {
        self.conn.close(0u32.into(), b"bye");
    }
}
