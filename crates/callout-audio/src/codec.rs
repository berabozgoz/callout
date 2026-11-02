//! Opus encode/decode wrapper. Constants come from [`callout_proto`]:
//! 48 kHz, mono, 20 ms (960 samples).
//!
//! Defaults are chosen for in-game voice: VOIP application, 32 kbps,
//! in-band FEC (assuming 10% loss), DTX (no packets during silence), complexity 8.

use callout_proto::{FRAME_SAMPLES, MAX_OPUS_PAYLOAD, SAMPLE_RATE_HZ};

pub const FRAME: usize = FRAME_SAMPLES as usize;

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("opus: {0}")]
    Opus(#[from] opus::Error),
    #[error("frame is {0} samples, expected {FRAME}")]
    BadFrameLen(usize),
}

#[derive(Debug, Clone, Copy)]
pub struct EncoderConfig {
    pub bitrate_bps: i32,
    pub fec: bool,
    /// libopus only produces FEC (LBRR) data if this value is high enough: at 32 kbps,
    /// FEC never kicks in at 5%; at 10% the lost-frame SNR goes ~5 → ~16 dB
    /// (`tests/loss_sim.rs`). Do not go below 10%.
    pub expected_loss_pct: i32,
    pub dtx: bool,
    /// 0–10; 10 is best quality / most CPU.
    pub complexity: i32,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            bitrate_bps: 32_000,
            fec: true,
            expected_loss_pct: 10,
            dtx: true,
            complexity: 8,
        }
    }
}

pub struct Encoder {
    inner: opus::Encoder,
    buf: [u8; MAX_OPUS_PAYLOAD],
}

impl Encoder {
    pub fn new(cfg: EncoderConfig) -> Result<Self, CodecError> {
        let mut inner = opus::Encoder::new(
            SAMPLE_RATE_HZ,
            opus::Channels::Mono,
            opus::Application::Voip,
        )?;
        inner.set_bitrate(opus::Bitrate::Bits(cfg.bitrate_bps))?;
        inner.set_vbr(true)?;
        inner.set_inband_fec(cfg.fec)?;
        inner.set_packet_loss_perc(cfg.expected_loss_pct)?;
        inner.set_dtx(cfg.dtx)?;
        inner.set_complexity(cfg.complexity)?;
        inner.set_signal(opus::Signal::Voice)?;
        Ok(Self {
            inner,
            buf: [0; MAX_OPUS_PAYLOAD],
        })
    }

    /// Encodes a 960-sample mono f32 frame; returns the Opus packet.
    /// If DTX decided on silence, the packet is 1–2 bytes and must not be sent
    /// (see [`Encoder::is_dtx_packet`]).
    pub fn encode(&mut self, frame: &[f32]) -> Result<&[u8], CodecError> {
        if frame.len() != FRAME {
            return Err(CodecError::BadFrameLen(frame.len()));
        }
        let n = self.inner.encode_float(frame, &mut self.buf)?;
        Ok(&self.buf[..n])
    }

    /// With DTX, Opus emits a 1–2 byte "empty" packet for silence.
    pub fn is_dtx_packet(packet: &[u8]) -> bool {
        packet.len() <= 2
    }
}

pub struct Decoder {
    inner: opus::Decoder,
}

impl Decoder {
    pub fn new() -> Result<Self, CodecError> {
        Ok(Self {
            inner: opus::Decoder::new(SAMPLE_RATE_HZ, opus::Channels::Mono)?,
        })
    }

    /// Decodes a packet into `out` (at least 960 samples). If `packet` is `None`,
    /// it is treated as packet loss and PLC produces 960 samples.
    /// Returns: number of samples written (always 960).
    pub fn decode(&mut self, packet: Option<&[u8]>, out: &mut [f32]) -> Result<usize, CodecError> {
        if out.len() < FRAME {
            return Err(CodecError::BadFrameLen(out.len()));
        }
        let n = self
            .inner
            .decode_float(packet.unwrap_or(&[]), &mut out[..FRAME], false)?;
        Ok(n)
    }

    /// Recovers the lost frame using the FEC data of the packet that follows the loss.
    /// Call this first (for the lost frame), then the normal `decode` (for the current frame).
    pub fn decode_fec(&mut self, next_packet: &[u8], out: &mut [f32]) -> Result<usize, CodecError> {
        if out.len() < FRAME {
            return Err(CodecError::BadFrameLen(out.len()));
        }
        Ok(self
            .inner
            .decode_float(next_packet, &mut out[..FRAME], true)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    fn sine(frames: usize, hz: f32) -> Vec<f32> {
        (0..frames * FRAME)
            .map(|i| 0.5 * (TAU * hz * i as f32 / SAMPLE_RATE_HZ as f32).sin())
            .collect()
    }

    #[test]
    fn roundtrip_keeps_signal_energy() {
        let mut enc = Encoder::new(EncoderConfig::default()).unwrap();
        let mut dec = Decoder::new().unwrap();
        let input = sine(25, 440.0);
        let mut out = vec![0f32; FRAME];
        let mut sizes = Vec::new();
        let mut energy_in = 0f32;
        let mut energy_out = 0f32;
        for (i, frame) in input.chunks(FRAME).enumerate() {
            let pkt = enc.encode(frame).unwrap().to_vec();
            sizes.push(pkt.len());
            let n = dec.decode(Some(&pkt), &mut out).unwrap();
            assert_eq!(n, FRAME);
            if i >= 5 {
                // skip codec warm-up
                energy_in += frame.iter().map(|s| s * s).sum::<f32>();
                energy_out += out.iter().map(|s| s * s).sum::<f32>();
            }
        }
        let ratio = energy_out / energy_in;
        assert!((0.5..2.0).contains(&ratio), "energy ratio {ratio}");
        let avg = sizes.iter().sum::<usize>() as f32 / sizes.len() as f32;
        // 32 kbps @ 20 ms ≈ 80 bytes; with VBR a sine compresses smaller.
        assert!(avg < 120.0, "average packet {avg} bytes");
    }

    #[test]
    fn dtx_emits_tiny_packets_on_silence() {
        let mut enc = Encoder::new(EncoderConfig::default()).unwrap();
        let silence = vec![0f32; FRAME];
        let mut last = 0;
        for _ in 0..60 {
            last = enc.encode(&silence).unwrap().len();
        }
        assert!(
            Encoder::is_dtx_packet(&vec![0u8; last]),
            "DTX packet size {last}"
        );
    }

    #[test]
    fn plc_fills_lost_frame() {
        let mut enc = Encoder::new(EncoderConfig::default()).unwrap();
        let mut dec = Decoder::new().unwrap();
        let input = sine(10, 300.0);
        let mut out = vec![0f32; FRAME];
        for frame in input.chunks(FRAME) {
            let pkt = enc.encode(frame).unwrap().to_vec();
            dec.decode(Some(&pkt), &mut out).unwrap();
        }
        let n = dec.decode(None, &mut out).unwrap();
        assert_eq!(n, FRAME);
        let energy: f32 = out.iter().map(|s| s * s).sum();
        assert!(energy > 1.0, "PLC produced silent output");
    }

    #[test]
    fn rejects_wrong_frame_len() {
        let mut enc = Encoder::new(EncoderConfig::default()).unwrap();
        assert!(matches!(
            enc.encode(&[0f32; 480]),
            Err(CodecError::BadFrameLen(480))
        ));
    }
}
