//! Simple energy-based voice activity detection (VAD).
//!
//! Opus DTX treats background hum as "sound"; this VAD sits before the codec and
//! never sends frames below the threshold. To avoid clipping words:
//! - **preroll**: when the gate opens, the few preceding frames are sent too
//!   (quiet consonants at word starts: s, sh, h, f);
//! - **hysteresis**: opening needs the threshold, staying open only `HYSTERESIS_DB` below it
//!   (softly spoken words are not cut mid-speech);
//! - **hangover**: sending continues for a while after speech ends (word endings).
//!
//! With noise suppression on, the VAD looks at the cleaned frame and also uses RNNoise's
//! speech probability (`process_with_prob`): a high probability opens the gate even below
//! the energy threshold. Push-to-talk bypasses all of this.

use crate::codec::FRAME;

/// An open gate stays open until the level drops this far below the open threshold.
pub const HYSTERESIS_DB: f32 = 4.0;
/// In adaptive mode the close threshold stays at least this far above the noise floor.
const MIN_CLOSE_ABOVE_FLOOR_DB: f32 = 3.0;
/// RNNoise speech probability: above this opens the gate; an open gate stays open above `PROB_HOLD`.
const PROB_OPEN: f32 = 0.85;
const PROB_HOLD: f32 = 0.5;

/// The user's sensitivity offset can lower the open threshold at most to this far above the noise
/// floor (adaptive mode), so a very sensitive setting cannot leave the gate open on plain noise.
const MIN_OPEN_ABOVE_FLOOR_DB: f32 = 4.0;

pub struct Vad {
    /// Open threshold (linear RMS); the close threshold is `HYSTERESIS_DB` below it.
    threshold: f32,
    /// Threshold before the user's offset (dBFS): fixed, or noise floor + margin in adaptive mode.
    base_db: f32,
    /// User sensitivity: added to `base_db` (negative = more sensitive).
    offset_db: f32,
    hangover_frames: u32,
    preroll_frames: u32,
    remaining: u32,
    speaking: bool,
    /// Adaptive mode: (noise floor dB, margin dB). The floor is slowly updated on
    /// non-speech frames; threshold = floor + margin.
    adaptive: Option<(f32, f32)>,
}

impl Vad {
    /// `threshold_db`: RMS threshold in dBFS (e.g. -45.0). `hangover_ms`: how long to stay
    /// open after speech. `preroll_ms`: how much past audio to send when the gate opens.
    pub fn new(threshold_db: f32, hangover_ms: u32, preroll_ms: u32) -> Self {
        Self {
            threshold: 10f32.powf(threshold_db / 20.0),
            base_db: threshold_db,
            offset_db: 0.0,
            hangover_frames: hangover_ms / 20,
            preroll_frames: preroll_ms / 20,
            remaining: 0,
            speaking: false,
            adaptive: None,
        }
    }

    /// Adaptive threshold: `floor_db` is the initial noise floor, `margin_db` the margin above it.
    pub fn set_adaptive(&mut self, floor_db: f32, margin_db: f32) {
        self.adaptive = Some((floor_db, margin_db));
        self.set_threshold_db((floor_db + margin_db).clamp(-60.0, -20.0));
    }

    /// User sensitivity in dB: positive lowers the open threshold (picks up quieter speech),
    /// negative raises it. Clamped to ±15 dB.
    pub fn set_sensitivity_db(&mut self, db: f32) {
        self.offset_db = -db.clamp(-15.0, 15.0);
        self.apply_threshold();
    }

    fn apply_threshold(&mut self) {
        let mut db = self.base_db + self.offset_db;
        if let Some((floor, _)) = self.adaptive {
            db = db.max(floor + MIN_OPEN_ABOVE_FLOOR_DB);
        }
        self.threshold = 10f32.powf(db / 20.0);
    }

    pub fn noise_floor_db(&self) -> Option<f32> {
        self.adaptive.map(|(f, _)| f)
    }

    /// Frame RMS (0.0–1.0).
    pub fn rms(frame: &[f32]) -> f32 {
        (frame.iter().map(|s| s * s).sum::<f32>() / FRAME as f32).sqrt()
    }

    /// Should this frame be sent? Also reports the start of speech
    /// (for the `AudioPacket` `TALK_START` flag).
    pub fn process(&mut self, frame: &[f32]) -> VadDecision {
        self.process_with_prob(frame, None)
    }

    /// [`Vad::process`] + the noise suppressor's speech probability (0.0–1.0).
    pub fn process_with_prob(&mut self, frame: &[f32], speech_prob: Option<f32>) -> VadDecision {
        let rms = Self::rms(frame);
        // Keep the close threshold away from the noise floor (fluctuating noise must not hold the gate open).
        let mut close = self.threshold * 10f32.powf(-HYSTERESIS_DB / 20.0);
        if let Some((floor, _)) = self.adaptive {
            close = close.max(10f32.powf((floor + MIN_CLOSE_ABOVE_FLOOR_DB) / 20.0));
        }
        let energy = rms >= if self.speaking { close } else { self.threshold };
        let prob = speech_prob.unwrap_or(0.0);
        let active = energy || prob >= if self.speaking { PROB_HOLD } else { PROB_OPEN };
        if let Some((floor, margin)) = self.adaptive.as_mut() {
            if !active && !self.speaking {
                // Floor: asymmetric EMA, only outside speech (not during hangover:
                // soft syllables below the threshold must not push the floor up toward speech).
                // Fast downward (follow immediately when the room gets quieter), slow upward.
                let db = Self::to_db(rms);
                let alpha = if db < *floor { 0.05 } else { 0.005 };
                *floor += (db - *floor) * alpha;
                self.base_db = (*floor + *margin).clamp(-60.0, -20.0);
                self.apply_threshold();
            }
        }
        let was_speaking = self.speaking;
        if active {
            self.remaining = self.hangover_frames;
            self.speaking = true;
        } else if self.remaining > 0 {
            self.remaining -= 1;
        } else {
            self.speaking = false;
        }
        let talk_start = self.speaking && !was_speaking;
        VadDecision {
            send: self.speaking,
            active,
            talk_start,
            preroll: if talk_start { self.preroll_frames } else { 0 },
        }
    }

    pub fn is_speaking(&self) -> bool {
        self.speaking
    }

    /// Sets the threshold before the user's sensitivity offset.
    pub fn set_threshold_db(&mut self, db: f32) {
        self.base_db = db;
        self.apply_threshold();
    }

    pub fn threshold_db(&self) -> f32 {
        20.0 * self.threshold.log10()
    }

    /// Linear RMS → dBFS (floor of -100 for silence).
    pub fn to_db(rms: f32) -> f32 {
        20.0 * rms.max(1e-5).log10()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VadDecision {
    pub send: bool,
    /// This frame itself carries speech (energy or speech probability); unlike `send` it is false
    /// during the hangover. The AGC only adapts on these frames.
    pub active: bool,
    pub talk_start: bool,
    /// If speech started: how many frames before this one should also be sent (preroll).
    pub preroll: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gates_silence_and_keeps_hangover() {
        let mut vad = Vad::new(-40.0, 60, 0); // 3 frames of hangover
        let silence = vec![0.0005f32; FRAME]; // ≈ -66 dBFS
        let voice = vec![0.1f32; FRAME]; // -20 dBFS
        assert!(!vad.process(&silence).send);
        let d = vad.process(&voice);
        assert!(d.send && d.talk_start);
        assert!(!vad.process(&voice).talk_start);
        assert!(vad.process(&silence).send);
        assert!(vad.process(&silence).send);
        assert!(vad.process(&silence).send);
        assert!(!vad.process(&silence).send);
    }

    #[test]
    fn preroll_only_at_talk_start_and_hysteresis_holds() {
        let mut vad = Vad::new(-40.0, 0, 60);
        let quiet = vec![0.0005f32; FRAME]; // ≈ -66 dBFS
        let loud = vec![0.02f32; FRAME]; // ≈ -34 dBFS
        let soft = vec![0.008f32; FRAME]; // ≈ -42 dBFS: does not open, but holds an open gate
        assert_eq!(vad.process(&soft).preroll, 0);
        assert!(!vad.process(&soft).send);
        let d = vad.process(&loud);
        assert!(d.talk_start);
        assert_eq!(d.preroll, 3);
        assert_eq!(vad.process(&loud).preroll, 0);
        assert!(
            vad.process(&soft).send,
            "hysteresis: open down to 4 dB below the threshold"
        );
        assert!(!vad.process(&quiet).send);
    }

    #[test]
    fn speech_probability_opens_and_holds_gate() {
        let mut vad = Vad::new(-40.0, 0, 0);
        let quiet = vec![0.0005f32; FRAME]; // far below the threshold
        assert!(
            !vad.process_with_prob(&quiet, Some(0.6)).send,
            "0.6 is not enough to open"
        );
        assert!(vad.process_with_prob(&quiet, Some(0.9)).talk_start);
        assert!(
            vad.process_with_prob(&quiet, Some(0.6)).send,
            "holds an open gate"
        );
        assert!(!vad.process_with_prob(&quiet, Some(0.2)).send);
    }
}

#[cfg(test)]
mod sim {
    use super::*;
    use std::f32::consts::TAU;

    /// Speech-like frame sequence: (frame, is speech). Noise floor is `noise_db`.
    pub fn speech(noise_db: f32, seconds: usize) -> Vec<(Vec<f32>, bool)> {
        let mut x: u64 = 12345;
        let mut rnd = move || {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((x >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
        };
        let noise_amp = 10f32.powf(noise_db / 20.0) * 3f32.sqrt(); // RMS correction for uniform distribution
        let syllable = [
            -45.0, -38.0, -32.0, -27.0, -26.0, -26.0, -28.0, -31.0, -36.0, -42.0,
        ];
        let mut env: Vec<Option<f32>> = Vec::new();
        let mut word = 0;
        while env.len() < seconds * 50 {
            for _ in 0..4 {
                // Every third word is soft; words start with a quiet consonant (s, sh, h).
                let soft = if word % 3 == 2 { -12.0 } else { 0.0 };
                env.extend([Some(-46.0 + soft / 2.0), Some(-43.0 + soft / 2.0)]);
                for _ in 0..2 {
                    env.extend(syllable.iter().map(|d| Some(d + soft)));
                    env.extend([None; 2]);
                }
                env.extend([None; 5]);
                word += 1;
            }
            env.extend([None; 40]);
        }
        let mut phase = 0f32;
        env.into_iter()
            .map(|e| {
                let amp = e.map(|d| 10f32.powf(d / 20.0) * 2f32.sqrt()).unwrap_or(0.0);
                let f: Vec<f32> = (0..FRAME)
                    .map(|_| {
                        phase += TAU * 160.0 / 48_000.0;
                        amp * phase.sin() + noise_amp * rnd()
                    })
                    .collect();
                (f, e.is_some())
            })
            .collect()
    }

    /// As the engine does: 75 frames of calibration (median), then adaptive VAD.
    /// Returns: (fraction of speech frames sent, fraction of non-speech frames sent).
    pub fn capture(noise_db: f32, hangover_ms: u32, preroll_ms: u32) -> (f32, f32) {
        let frames = speech(noise_db, 60);
        let mut vad = Vad::new(-45.0, hangover_ms, preroll_ms);
        // Calibration happens during silence (while the app starts).
        let mut calib: Vec<f32> = frames
            .iter()
            .filter(|f| !f.1)
            .take(75)
            .map(|f| Vad::to_db(Vad::rms(&f.0)))
            .collect();
        calib.sort_by(|a, b| a.partial_cmp(b).unwrap());
        vad.set_adaptive(calib[calib.len() / 2], 6.0);
        let mut sent = vec![false; frames.len()];
        for (i, (f, _)) in frames.iter().enumerate() {
            let d = vad.process(f);
            if d.send {
                sent[i] = true;
                for j in 0..d.preroll as usize {
                    if i > j {
                        sent[i - 1 - j] = true;
                    }
                }
            }
        }
        let ratio = |speech: bool| {
            let idx: Vec<usize> = (0..frames.len())
                .filter(|&i| frames[i].1 == speech)
                .collect();
            idx.iter().filter(|&&i| sent[i]).count() as f32 / idx.len() as f32
        };
        (ratio(true), ratio(false))
    }

    /// On a noisy mic (−42 dB floor; the first version sent only 68% of speech)
    /// soft words and word starts must get through, and noise must not hold the gate open.
    #[test]
    fn keeps_soft_speech_without_locking_open() {
        for (noise, min_speech) in [(-60.0, 0.99), (-50.0, 0.99), (-42.0, 0.9)] {
            let (speech, other) = capture(noise, 400, 60);
            assert!(
                speech >= min_speech,
                "floor {noise}: speech {:.1}%",
                speech * 100.0
            );
            assert!(
                other < 0.75,
                "floor {noise}: non-speech {:.1}%",
                other * 100.0
            );
        }
    }

    #[test]
    #[ignore = "experiment: VAD tuning sweep"]
    fn report() {
        for n in [-60.0, -50.0, -42.0] {
            for (h, p) in [(300, 0), (300, 60), (400, 60), (500, 60)] {
                let (a, b) = capture(n, h, p);
                println!(
                    "floor {n} dB, hangover {h} ms, preroll {p} ms: speech {:.1}%, non-speech {:.1}%",
                    a * 100.0,
                    b * 100.0
                );
            }
        }
    }

    #[test]
    fn sensitivity_moves_threshold_but_not_below_noise() {
        let mut vad = Vad::new(-45.0, 0, 0);
        vad.set_adaptive(-42.0, 6.0); // threshold -36
        assert!((vad.threshold_db() + 36.0).abs() < 0.01);
        vad.set_sensitivity_db(1.0);
        assert!(
            (vad.threshold_db() + 37.0).abs() < 0.01,
            "{}",
            vad.threshold_db()
        );
        vad.set_sensitivity_db(-6.0);
        assert!((vad.threshold_db() + 30.0).abs() < 0.01);
        // Very sensitive: capped 4 dB above the -42 dB floor.
        vad.set_sensitivity_db(15.0);
        assert!(
            (vad.threshold_db() + 38.0).abs() < 0.01,
            "{}",
            vad.threshold_db()
        );
    }
}
