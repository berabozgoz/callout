//! Serbest uzunluklu örnek akışını sabit 960 örneklik (20 ms @ 48 kHz) çerçevelere böler.

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

    /// Hazır bir çerçeve varsa `out`'a yazar ve `true` döner.
    pub fn next_frame(&mut self, out: &mut [f32; FRAME]) -> bool {
        if self.buf.len() < FRAME {
            return false;
        }
        out.copy_from_slice(&self.buf[..FRAME]);
        self.buf.drain(..FRAME);
        true
    }

    /// Bekleyen (henüz tam çerçeve olmayan) örnek sayısı.
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
