//! Client-side QUIC connection and handshake.
//!
//! Flow: `Hello` → `Welcome{challenge}` → `Auth{signature}` → `Authed`.
//! After that, room messages go through [`Client::control`] and audio datagrams through
//! [`Client::send_audio`] / [`Client::recv_audio`].

use crate::cert::{Invite, PinnedVerifier};
use crate::control::{ControlError, ControlStream};
use crate::identity::DeviceKey;
use crate::{frame_control, transport_config, unframe_control, NetError, ALPN, MAX_CONTROL_FRAME};
use bytes::Bytes;
use callout_proto::{
    ClientMsg, ErrorCode, LogUpload, LogUploadReply, ServerMsg, UserId, PROTOCOL_VERSION,
};
use std::net::SocketAddr;
use std::sync::Arc;

/// The QUIC connection handle [`Client::connection`] returns, for crates that do not depend on quinn.
pub use quinn::Connection;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("local UDP socket: {0}")]
    Socket(#[from] std::io::Error),
    #[error("TLS configuration: {0}")]
    Tls(#[from] rustls::Error),
    #[error("QUIC configuration: {0}")]
    QuicConfig(#[from] quinn::crypto::rustls::NoInitialCipherSuite),
    #[error("could not connect: {0}")]
    Connect(#[from] quinn::ConnectError),
    #[error("connection lost: {0}")]
    Connection(#[from] quinn::ConnectionError),
    #[error(transparent)]
    Control(#[from] ControlError),
    #[error("server rejected: {code:?}: {message}")]
    Rejected { code: ErrorCode, message: String },
    #[error("unexpected message during handshake: {0:?}")]
    Unexpected(ServerMsg),
    #[error("failed to send datagram: {0}")]
    Datagram(#[from] quinn::SendDatagramError),
}

#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("connection: {0}")]
    Connection(#[from] quinn::ConnectionError),
    #[error("stream write: {0}")]
    Write(#[from] quinn::WriteError),
    #[error("stream read: {0}")]
    Read(#[from] quinn::ReadToEndError),
    #[error("stream closed")]
    Closed,
    #[error(transparent)]
    Net(#[from] NetError),
}

pub struct Client {
    conn: quinn::Connection,
    control: ControlStream<ClientMsg, ServerMsg>,
    user_id: UserId,
    /// Used to wait for the close frame to go out on the wire (`close_graceful`).
    endpoint: quinn::Endpoint,
}

impl Client {
    /// Connects to the server in the invite link and completes authentication.
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

        tracing::info!(user_id, rtt_ms = conn.rtt().as_millis(), "connected");
        Ok(Self {
            conn,
            control,
            user_id,
            endpoint,
        })
    }

    pub fn user_id(&self) -> UserId {
        self.user_id
    }

    pub fn control(&mut self) -> &mut ControlStream<ClientMsg, ServerMsg> {
        &mut self.control
    }

    /// Creates a room; the reply arrives on the control stream as `RoomState` or `Error`.
    pub async fn create_room(&mut self, name: &str) -> Result<(), ClientError> {
        Ok(self
            .control
            .send(&ClientMsg::CreateRoom {
                name: name.to_string(),
            })
            .await?)
    }

    /// Joins a room by code; the reply is `RoomState` or `Error`.
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

    /// Closes and deletes the current room (owner only).
    pub async fn close_room(&mut self) -> Result<(), ClientError> {
        Ok(self.control.send(&ClientMsg::CloseRoom).await?)
    }

    pub fn rtt(&self) -> std::time::Duration {
        self.conn.rtt()
    }

    /// Sends a ready audio packet (header + Opus) as an unreliable datagram.
    pub fn send_audio(&self, packet: Bytes) -> Result<(), ClientError> {
        Ok(self.conn.send_datagram(packet)?)
    }

    /// Waits for the next audio datagram.
    pub async fn recv_audio(&self) -> Result<Bytes, ClientError> {
        Ok(self.conn.read_datagram().await?)
    }

    /// An independent handle for receiving datagrams (for use in a separate task).
    pub fn connection(&self) -> quinn::Connection {
        self.conn.clone()
    }

    pub fn close(&self) {
        self.conn.close(0u32.into(), b"bye");
    }

    /// Sends a diagnostic log on a stream of its own (see [`callout_proto::LogUpload`]) and
    /// waits for the server's reply. A server without upload support never answers, so the
    /// caller puts a timeout around this. Takes the connection (from [`Client::connection`]) so
    /// it can run in a task of its own while the control loop goes on.
    pub async fn upload_log(
        conn: &quinn::Connection,
        app_version: &str,
        log: &[u8],
    ) -> Result<LogUploadReply, UploadError> {
        let (mut send, mut recv) = conn.open_bi().await?;
        let mut head = Vec::new();
        frame_control(
            &LogUpload {
                app_version: app_version.to_string(),
                bytes: log.len() as u32,
            },
            &mut head,
        )?;
        send.write_all(&head).await?;
        send.write_all(log).await?;
        send.finish().map_err(|_| UploadError::Closed)?;
        let reply = recv.read_to_end(MAX_CONTROL_FRAME).await?;
        Ok(unframe_control::<LogUploadReply>(&reply)?.0)
    }

    /// Closes the connection and waits (at most `wait`) for the close frame to be sent.
    /// If the process exits right after, the frame is not sent and the server drops the connection on timeout after ~10 s.
    pub async fn close_graceful(&self, wait: std::time::Duration) {
        self.conn.close(0u32.into(), b"bye");
        let _ = tokio::time::timeout(wait, self.endpoint.wait_idle()).await;
    }
}
