//! Control stream: length-prefixed `postcard` messages over a bidirectional QUIC stream.
//! The client uses `ControlStream<ClientMsg, ServerMsg>`, the server `ControlStream<ServerMsg, ClientMsg>`.

use crate::{frame_control, unframe_control, NetError, MAX_CONTROL_FRAME};
use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;

#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error(transparent)]
    Net(#[from] NetError),
    #[error("stream write: {0}")]
    Write(#[from] quinn::WriteError),
    #[error("stream read: {0}")]
    Read(#[from] quinn::ReadError),
    #[error("peer closed the control stream")]
    Closed,
}

pub struct ControlStream<Tx, Rx> {
    send: quinn::SendStream,
    recv: quinn::RecvStream,
    out: Vec<u8>,
    inbuf: Vec<u8>,
    _t: PhantomData<(Tx, Rx)>,
}

impl<Tx: Serialize, Rx: DeserializeOwned> ControlStream<Tx, Rx> {
    pub fn new(send: quinn::SendStream, recv: quinn::RecvStream) -> Self {
        Self {
            send,
            recv,
            out: Vec::with_capacity(256),
            inbuf: Vec::with_capacity(1024),
            _t: PhantomData,
        }
    }

    pub async fn send(&mut self, msg: &Tx) -> Result<(), ControlError> {
        self.out.clear();
        frame_control(msg, &mut self.out)?;
        self.send.write_all(&self.out).await?;
        Ok(())
    }

    /// Waits for the next complete message.
    pub async fn recv(&mut self) -> Result<Rx, ControlError> {
        loop {
            match unframe_control::<Rx>(&self.inbuf) {
                Ok((msg, used)) => {
                    self.inbuf.drain(..used);
                    return Ok(msg);
                }
                Err(NetError::Incomplete(_)) => {}
                Err(e) => return Err(e.into()),
            }
            let mut chunk = [0u8; 2048];
            let n = self
                .recv
                .read(&mut chunk)
                .await?
                .ok_or(ControlError::Closed)?;
            self.inbuf.extend_from_slice(&chunk[..n]);
            if self.inbuf.len() > MAX_CONTROL_FRAME + 4 {
                return Err(NetError::FrameTooLarge(self.inbuf.len()).into());
            }
        }
    }

    /// Closes the stream cleanly (sends FIN).
    pub async fn finish(&mut self) -> Result<(), ControlError> {
        self.send.finish().map_err(|_| ControlError::Closed)?;
        Ok(())
    }
}
