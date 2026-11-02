//! Splits a sample stream of arbitrary length into fixed 960-sample (20 ms @ 48 kHz) frames.

use crate::codec::FRAME;

#[derive(Default)]
pub struct Framer {
    buf: Vec<f32>,
}

impl Framer {
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(FRAME * 4),
        }
    }

    pub fn push(&mut self, samples: &[f32]) {
        self.buf.extend_from_slice(samples);
    }

    /// If a full frame is ready, writes it to `out` and returns `true`.
    pub fn next_frame(&mut self, out: &mut [f32; FRAME]) -> bool {
        if self.buf.len() < FRAME {
            return false;
        }
        out.copy_from_slice(&self.buf[..FRAME]);
        self.buf.drain(..FRAME);
        true
    }

    /// Number of pending samples (not yet a full frame).
    pub fn pending(&self) -> usize {
        self.buf.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_into_exact_frames() {
        let mut f = Framer::new();
        let mut frame = [0f32; FRAME];
        f.push(&vec![1.0; 500]);
        assert!(!f.next_frame(&mut frame));
        f.push(&vec![2.0; 1500]);
        assert!(f.next_frame(&mut frame));
        assert_eq!(frame[0], 1.0);
        assert_eq!(frame[FRAME - 1], 2.0);
        assert!(f.next_frame(&mut frame));
        assert!(!f.next_frame(&mut frame));
        assert_eq!(f.pending(), 80);
    }
}
