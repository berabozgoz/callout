//! Acoustic echo cancellation: WebRTC's AEC3 through `sonora`, its pure-Rust port.
//!
//! A player's microphone picks up what their own headphones or speakers play: the other
//! players' voices. Without cancellation those go back into the room and the others hear
//! themselves as an echo. The canceller is given what we play ([`EchoCanceller::render`]) and
//! subtracts its estimated echo from the microphone ([`EchoCanceller::capture`]); it learns the
//! echo path (delay, level, room) on its own and stays transparent when there is no echo.
//!
//! The echo is created at the listener's PC, so each client cancels the echo of what *it* plays.
//! AEC3 works on 10 ms blocks; a 20 ms frame is processed in two halves. Runs on the audio
//! thread (not in the real-time callback): it allocates at setup only.

use crate::codec::FRAME;
use sonora::config::EchoCanceller as EchoCancellerConfig;
use sonora::{AudioProcessing, Config, StreamConfig};

const RATE: u32 = 48_000;
const HALF: usize = FRAME / 2;

/// Echo statistics from the canceller; `None` until it has an estimate.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EchoStats {
    /// Echo return loss (dB): how much quieter the echo is than what was played (the
    /// acoustic path; headphones give a high value, speakers a low one).
    pub erl_db: Option<f64>,
    /// Echo return loss enhancement (dB): how much the canceller removed.
    pub erle_db: Option<f64>,
    /// The canceller's estimate of the delay between playing and hearing (ms).
    pub delay_ms: Option<i32>,
}

pub struct EchoCanceller {
    apm: AudioProcessing,
    out: [f32; HALF],
}

impl Default for EchoCanceller {
    fn default() -> Self {
        Self::new()
    }
}

impl EchoCanceller {
    pub fn new() -> Self {
        // Echo cancellation only: noise suppression (RNNoise) and AGC are our own, later in the
        // chain. The canceller's high-pass filter (below ~80 Hz) stays on, as AEC3 expects.
        let config = Config {
            echo_canceller: Some(EchoCancellerConfig::default()),
            ..Default::default()
        };
        let apm = AudioProcessing::builder()
            .config(config)
            .capture_config(StreamConfig::new(RATE, 1))
            .render_config(StreamConfig::new(RATE, 1))
            .build();
        Self {
            apm,
            out: [0.0; HALF],
        }
    }

    /// A 960-sample frame that is about to be played (the final mix, 48 kHz mono).
    pub fn render(&mut self, frame: &[f32]) {
        debug_assert_eq!(frame.len(), FRAME);
        for half in frame.as_chunks::<HALF>().0 {
            // The render output is unused (nothing is applied to the played signal).
            let _ = self
                .apm
                .process_render_f32(&[half], &mut [&mut self.out[..]]);
        }
    }

    /// Removes the echo from a 960-sample microphone frame (48 kHz mono), in place.
    pub fn capture(&mut self, frame: &mut [f32]) {
        debug_assert_eq!(frame.len(), FRAME);
        for half in frame.as_chunks_mut::<HALF>().0 {
            if self
                .apm
                .process_capture_f32(&[&*half], &mut [&mut self.out[..]])
                .is_ok()
            {
                half.copy_from_slice(&self.out);
            }
        }
    }

    /// Hint for the delay between handing a frame to [`EchoCanceller::render`] and handing its
    /// echo to [`EchoCanceller::capture`] (ms): output queue + device buffers. AEC3 searches the
    /// real delay itself; the hint only has to be in the right range.
    pub fn set_delay_ms(&mut self, ms: u32) {
        let _ = self.apm.set_stream_delay_ms(ms.min(500) as i32);
    }

    pub fn stats(&self) -> EchoStats {
        let s = self.apm.statistics();
        EchoStats {
            erl_db: s.echo_return_loss,
            erle_db: s.echo_return_loss_enhancement,
            delay_ms: s.delay_ms,
        }
    }
}
