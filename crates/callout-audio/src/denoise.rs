//! Noise suppression: `nnnoiseless`, the pure-Rust port of RNNoise.
//!
//! Removes steady noise such as fans, hum and room tone; widens the gap between speech and
//! noise on mics far from the mouth, like webcam or laptop mics. As a by-product it gives a
//! "speech probability" per frame; the VAD uses it in addition to energy.
//!
//! RNNoise works on 10 ms (480 samples); a 20 ms frame is processed in two halves. Input is
//! expected in `i16` range (±32768), hence the scaling. Runs on the audio thread
//! (not in the real-time callback); allocation only at setup.

use crate::codec::FRAME;
use nnnoiseless::DenoiseState;

const HALF: usize = DenoiseState::FRAME_SIZE;
const SCALE: f32 = 32768.0;

pub struct Denoiser {
    state: Box<DenoiseState<'static>>,
    inp: [f32; HALF],
    out: [f32; HALF],
}

impl Default for Denoiser {
    fn default() -> Self {
        Self::new()
    }
}

impl Denoiser {
    pub fn new() -> Self {
        debug_assert_eq!(FRAME, 2 * HALF);
        Self {
            state: DenoiseState::new(),
            inp: [0.0; HALF],
            out: [0.0; HALF],
        }
    }

    /// Cleans a 960-sample frame in place; returns the higher speech probability of the
    /// two halves (0.0–1.0). Output is delayed by RNNoise's window overlap (~10 ms).
    pub fn process(&mut self, frame: &mut [f32]) -> f32 {
        let mut prob = 0f32;
        for half in frame.as_chunks_mut::<HALF>().0 {
            for (d, s) in self.inp.iter_mut().zip(half.iter()) {
                *d = s * SCALE;
            }
            prob = prob.max(self.state.process_frame(&mut self.out, &self.inp));
            for (d, s) in half.iter_mut().zip(self.out.iter()) {
                *d = (s / SCALE).clamp(-1.0, 1.0);
            }
        }
        prob
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt()
    }

    fn db(x: f32) -> f32 {
        20.0 * x.max(1e-6).log10()
    }

    #[test]
    fn suppresses_steady_noise() {
        // ~−42 dBFS low-passed (pinkish) noise: Bera's webcam mic noise floor, 2 s.
        // Room/fan noise is low-frequency; RNNoise suppresses loud white noise only slightly.
        let mut x: u64 = 99;
        let mut lp = 0f32;
        let amp = 6.0 * 10f32.powf(-42.0 / 20.0) * 3f32.sqrt();
        let mut d = Denoiser::new();
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut max_prob = 0f32;
        for i in 0..100 {
            let mut f: Vec<f32> = (0..FRAME)
                .map(|_| {
                    x = x
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    lp += ((((x >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0) - lp) * 0.05;
                    amp * lp
                })
                .collect();
            let raw = rms(&f);
            let p = d.process(&mut f);
            assert!(f.iter().all(|s| s.is_finite()));
            if i >= 25 {
                // first 0.5 s is adaptation time
                before.push(raw);
                after.push(rms(&f));
                max_prob = max_prob.max(p);
            }
        }
        let b = db(before.iter().sum::<f32>() / before.len() as f32);
        let a = db(after.iter().sum::<f32>() / after.len() as f32);
        assert!(b > -46.0 && b < -38.0, "test noise {b:.1} dB");
        assert!(a < b - 30.0, "noise {b:.1} dB → {a:.1} dB");
        assert!(max_prob < 0.5, "speech probability on noise {max_prob}");
    }
}
