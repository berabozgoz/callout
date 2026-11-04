//! Automatic gain control (AGC): evens out the speaking level of the microphone signal.
//!
//! A quiet mic (webcam, laptop) and a loud one end up at about the same level for the listeners,
//! and a player who whispers one moment and shouts the next does not need constant volume
//! changes. Runs on the audio thread after noise suppression and voice detection, before the
//! frame goes into the pre-roll history and the encoder. The VAD and the level meter see the
//! signal *before* the gain, so their thresholds do not move with it.
//!
//! The gain is kept in dB and only moves on frames that carry speech (`active`), so noise
//! between words is never pumped up:
//! - **level**: the speech RMS is smoothed (fast up, slow down) and the gain aims at
//!   `TARGET_DB - level`;
//! - **peaks**: the speech *peak* is tracked too (instant up, slow down) and the gain never
//!   rises above what keeps those peaks `PEAK_CEIL_DB` below full scale. Speech has a high crest
//!   factor, so a gain that only follows the RMS would push every loud syllable into the limiter;
//! - **limiter** (backstop): when a frame would still pass `LIMIT`, a separate, quickly recovering
//!   reduction is applied (it is not written into the gain, so one pop does not drag the gain
//!   down for seconds); a soft knee is the last safety net so nothing ever clips.
//!
//! Gain changes ramp over `RAMP_SAMPLES` (2 ms): fast enough for the limiter, smooth enough to
//! avoid clicks.

/// Speech RMS the level loop aims for (dBFS). Leaves room for the crest factor of speech.
pub const TARGET_DB: f32 = -23.0;
pub const MIN_GAIN_DB: f32 = -10.0;
/// Highest gain with noise suppression on; without it the room noise between words would be
/// amplified too, so the caller lowers the ceiling (see `set_max_gain_db`). A very quiet mic
/// would want more, but every dB also raises the denoiser's residue and the mic's own noise.
pub const MAX_GAIN_DB: f32 = 12.0;
/// Where the tracked speech peaks may end up (dBFS).
const PEAK_CEIL_DB: f32 = -6.0;
/// Peak level (linear) the output is kept under.
const LIMIT: f32 = 0.89;

const FRAME_MS: f32 = 20.0;
/// Level detector time constants: a louder frame is followed fast, a quieter one slowly (so the
/// gain does not follow every syllable).
const LEVEL_UP_MS: f32 = 50.0;
const LEVEL_DOWN_MS: f32 = 400.0;
/// The tracked speech peak: follows a louder peak within a few frames (a single pop only moves it
/// part of the way) and falls back slowly.
const PEAK_UP_MS: f32 = 40.0;
const PEAK_DOWN_MS: f32 = 1500.0;
/// Gain time constants: down (speaker got louder), up, and up during the first speech.
const ATTACK_MS: f32 = 100.0;
const RELEASE_MS: f32 = 1500.0;
const FAST_RELEASE_MS: f32 = 300.0;
/// How fast the limiter's reduction recovers.
const LIMITER_RELEASE_MS: f32 = 80.0;
/// Active frames (2 s of speech) that count as "the first speech" after a reset.
const FAST_START_FRAMES: u32 = 100;
/// Samples (48 kHz) over which a gain change is ramped.
const RAMP_SAMPLES: usize = 96;

/// What the AGC did since the last `take_stats` (for the log).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Stats {
    /// Frames processed.
    pub frames: u32,
    /// Frames in which the limiter had to act.
    pub limited_frames: u32,
    /// Highest output sample (linear).
    pub peak_out: f32,
}

pub struct Agc {
    /// Gain from the level/peak loop (dB).
    gain_db: f32,
    /// Extra reduction by the limiter (dB, ≤ 0).
    limit_db: f32,
    /// Total gain at the end of the previous frame (linear), where the next ramp starts.
    prev_total: f32,
    max_gain_db: f32,
    /// Smoothed speech level and tracked speech peak (dBFS) before the gain; `None` until the
    /// first speech frame.
    level_db: Option<f32>,
    peak_db: Option<f32>,
    active_frames: u32,
    stats: Stats,
    level_up: f32,
    level_down: f32,
    peak_up: f32,
    peak_down: f32,
    attack: f32,
    release: f32,
    fast_release: f32,
    limiter_release: f32,
}

impl Default for Agc {
    fn default() -> Self {
        Self::new()
    }
}

/// One-pole smoothing coefficient for a time constant, per 20 ms frame.
fn coeff(tau_ms: f32) -> f32 {
    1.0 - (-FRAME_MS / tau_ms).exp()
}

fn to_db(x: f32) -> f32 {
    20.0 * x.max(1e-5).log10()
}

fn from_db(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Passes samples up to `LIMIT` unchanged, then bends smoothly toward 1.0 (never above it).
fn soft_limit(x: f32) -> f32 {
    let a = x.abs();
    if a <= LIMIT {
        return x;
    }
    let knee = 1.0 - LIMIT;
    (LIMIT + knee * ((a - LIMIT) / knee).tanh()).min(1.0) * x.signum()
}

impl Agc {
    pub fn new() -> Self {
        Self {
            gain_db: 0.0,
            limit_db: 0.0,
            prev_total: 1.0,
            max_gain_db: MAX_GAIN_DB,
            level_db: None,
            peak_db: None,
            active_frames: 0,
            stats: Stats::default(),
            level_up: coeff(LEVEL_UP_MS),
            level_down: coeff(LEVEL_DOWN_MS),
            peak_up: coeff(PEAK_UP_MS),
            peak_down: coeff(PEAK_DOWN_MS),
            attack: coeff(ATTACK_MS),
            release: coeff(RELEASE_MS),
            fast_release: coeff(FAST_RELEASE_MS),
            limiter_release: coeff(LIMITER_RELEASE_MS),
        }
    }

    /// Back to unity gain and no level history (when the AGC is switched on again).
    pub fn reset(&mut self) {
        *self = Self {
            max_gain_db: self.max_gain_db,
            ..Self::new()
        };
    }

    /// Ceiling for the gain (dB), e.g. lower without noise suppression.
    pub fn set_max_gain_db(&mut self, db: f32) {
        self.max_gain_db = db.clamp(0.0, MAX_GAIN_DB);
    }

    /// The gain of the level/peak loop (dB), without the limiter's momentary reduction.
    pub fn gain_db(&self) -> f32 {
        self.gain_db
    }

    /// Returns what happened since the last call and starts counting again.
    pub fn take_stats(&mut self) -> Stats {
        std::mem::take(&mut self.stats)
    }

    /// Applies the gain to a frame in place. `active`: the frame carries speech (the VAD's
    /// decision for this frame, not the hangover).
    pub fn process(&mut self, frame: &mut [f32], active: bool) {
        if frame.is_empty() {
            return;
        }
        let peak = frame.iter().fold(0f32, |m, s| m.max(s.abs()));

        if active {
            let rms = (frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32).sqrt();
            if rms > 1e-5 {
                let level = to_db(rms);
                let level = match self.level_db {
                    None => level,
                    Some(l) => {
                        l + (level - l)
                            * if level > l {
                                self.level_up
                            } else {
                                self.level_down
                            }
                    }
                };
                self.level_db = Some(level);
                let p = to_db(peak);
                let tracked = match self.peak_db {
                    None => p,
                    Some(t) => t + (p - t) * if p > t { self.peak_up } else { self.peak_down },
                };
                self.peak_db = Some(tracked);

                let desired = (TARGET_DB - level)
                    .min(PEAK_CEIL_DB - tracked)
                    .clamp(MIN_GAIN_DB, self.max_gain_db);
                let alpha = if desired < self.gain_db {
                    self.attack
                } else if self.active_frames < FAST_START_FRAMES {
                    self.fast_release
                } else {
                    self.release
                };
                self.gain_db += (desired - self.gain_db) * alpha;
                self.active_frames = self.active_frames.saturating_add(1);
            }
        }
        self.gain_db = self.gain_db.clamp(MIN_GAIN_DB, self.max_gain_db);

        // Limiter backstop: reduce just enough for this frame's peak, recover quickly after.
        let needed = (to_db(LIMIT / peak.max(1e-5)) - self.gain_db).min(0.0);
        self.limit_db = if needed < self.limit_db {
            needed
        } else {
            (self.limit_db * (1.0 - self.limiter_release)).min(needed)
        };
        let limited = self.limit_db < -0.05;

        let start = self.prev_total;
        let end = from_db(self.gain_db + self.limit_db);
        let mut soft_hit = false;
        let mut peak_out = 0f32;
        for (i, s) in frame.iter_mut().enumerate() {
            let t = ((i + 1) as f32 / RAMP_SAMPLES as f32).min(1.0);
            let y = *s * (start + (end - start) * t);
            soft_hit |= y.abs() > LIMIT;
            *s = soft_limit(y).clamp(-1.0, 1.0);
            peak_out = peak_out.max(s.abs());
        }
        self.prev_total = end;

        self.stats.frames += 1;
        self.stats.limited_frames += u32::from(limited || soft_hit);
        self.stats.peak_out = self.stats.peak_out.max(peak_out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::FRAME;
    use std::f32::consts::TAU;

    /// Sine frame at `rms_db` dBFS RMS (160 Hz, phase continues across calls).
    fn tone(rms_db: f32, phase: &mut f32) -> Vec<f32> {
        let amp = from_db(rms_db) * 2f32.sqrt();
        (0..FRAME)
            .map(|_| {
                *phase += TAU * 160.0 / 48_000.0;
                amp * phase.sin()
            })
            .collect()
    }

    fn rms_db(f: &[f32]) -> f32 {
        to_db((f.iter().map(|s| s * s).sum::<f32>() / f.len() as f32).sqrt())
    }

    fn peak(f: &[f32]) -> f32 {
        f.iter().fold(0f32, |m, s| m.max(s.abs()))
    }

    #[test]
    fn brings_quiet_speech_up_to_target() {
        let mut agc = Agc::new();
        let mut ph = 0.0;
        let mut last = Vec::new();
        for _ in 0..150 {
            last = tone(-32.0, &mut ph);
            agc.process(&mut last, true);
        }
        let out = rms_db(&last);
        assert!((out - TARGET_DB).abs() < 2.0, "{out:.1} dBFS");
        let want = TARGET_DB + 32.0;
        assert!(
            (agc.gain_db() - want).abs() < 2.0,
            "{:.1} dB",
            agc.gain_db()
        );
    }

    #[test]
    fn brings_loud_speech_down_without_clipping() {
        let mut agc = Agc::new();
        let mut ph = 0.0;
        let mut last = Vec::new();
        for i in 0..150 {
            // −3 dBFS RMS: the peak is full scale.
            last = tone(-3.0, &mut ph);
            agc.process(&mut last, true);
            assert!(peak(&last) <= 1.0, "frame {i}");
            if i > 0 {
                assert!(peak(&last) <= LIMIT + 0.01, "frame {i}: {}", peak(&last));
            }
        }
        assert!(agc.gain_db() < -5.0, "{:.1} dB", agc.gain_db());
        assert!(rms_db(&last) < -12.0, "{:.1} dBFS", rms_db(&last));
    }

    #[test]
    fn frames_without_speech_do_not_change_the_gain() {
        let mut agc = Agc::new();
        let mut ph = 0.0;
        for _ in 0..150 {
            let mut f = tone(-32.0, &mut ph);
            agc.process(&mut f, true);
        }
        let before = agc.gain_db();
        // Room noise at −60 dBFS (denoised floor is lower still): flagged as not speech.
        for _ in 0..500 {
            let mut f = tone(-60.0, &mut ph);
            agc.process(&mut f, false);
        }
        assert_eq!(
            agc.gain_db(),
            before,
            "the gain must stay where speech left it"
        );
    }

    #[test]
    fn sudden_shout_is_caught_within_a_frame() {
        let mut agc = Agc::new();
        let mut ph = 0.0;
        for _ in 0..150 {
            let mut f = tone(-32.0, &mut ph);
            agc.process(&mut f, true);
        }
        assert!(agc.gain_db() > 7.0);
        // From a murmur to −10 dBFS RMS with +9 dB gain would be far over full scale.
        for i in 0..20 {
            let mut f = tone(-10.0, &mut ph);
            agc.process(&mut f, true);
            assert!(peak(&f) <= 1.0, "frame {i} clipped");
            if i > 0 {
                assert!(peak(&f) <= LIMIT + 0.01, "frame {i}: {}", peak(&f));
            }
        }
    }

    #[test]
    fn gain_stays_within_limits() {
        let mut agc = Agc::new();
        let mut ph = 0.0;
        // Very quiet speech: wants +27 dB, gets the ceiling.
        for _ in 0..500 {
            let mut f = tone(-50.0, &mut ph);
            agc.process(&mut f, true);
        }
        assert!(
            (agc.gain_db() - MAX_GAIN_DB).abs() < 0.1,
            "{:.1}",
            agc.gain_db()
        );
        // Without noise suppression the ceiling is lower; the gain follows it down at once.
        agc.set_max_gain_db(6.0);
        let mut f = tone(-50.0, &mut ph);
        agc.process(&mut f, true);
        assert!(agc.gain_db() <= 6.0 + 1e-3, "{:.1}", agc.gain_db());
        for _ in 0..500 {
            let mut f = tone(-3.0, &mut ph);
            agc.process(&mut f, true);
        }
        assert!(agc.gain_db() >= MIN_GAIN_DB - 1e-3, "{:.1}", agc.gain_db());
    }

    #[test]
    fn peaky_speech_gets_less_gain_instead_of_hitting_the_limiter() {
        // A quiet tone (−35 dBFS RMS) with a sharp peak at −6 dBFS in every fifth frame, like the
        // plosives and loud vowels of real speech. The RMS alone would ask for +12 dB, which would
        // put those peaks 6 dB over full scale.
        let mut agc = Agc::new();
        let mut ph = 0.0;
        let mut run = |agc: &mut Agc, frames: usize| {
            for i in 0..frames {
                let mut f = tone(-35.0, &mut ph);
                if i % 5 == 0 {
                    f[480] = 0.5;
                }
                agc.process(&mut f, true);
            }
        };
        run(&mut agc, 200);
        agc.take_stats();
        run(&mut agc, 200);
        assert!(agc.gain_db() < 4.0, "{:.1} dB", agc.gain_db());
        let stats = agc.take_stats();
        assert_eq!(stats.limited_frames, 0, "{stats:?}");
        assert!(stats.peak_out < LIMIT, "{stats:?}");
    }

    #[test]
    fn a_single_pop_does_not_drag_the_gain_down_for_long() {
        let mut agc = Agc::new();
        let mut ph = 0.0;
        for _ in 0..300 {
            let mut f = tone(-32.0, &mut ph);
            agc.process(&mut f, true);
        }
        let before = agc.gain_db();
        let mut pop = tone(-32.0, &mut ph);
        pop[100] = 1.0;
        agc.process(&mut pop, true);
        assert!(peak(&pop) <= 1.0);
        // The limiter's own reduction is gone within half a second...
        for _ in 0..25 {
            let mut f = tone(-32.0, &mut ph);
            agc.process(&mut f, true);
        }
        assert!(agc.limit_db > -0.1, "{:.2} dB", agc.limit_db);
        // ...and the loop's gain comes back too, instead of staying down.
        for _ in 0..400 {
            let mut f = tone(-32.0, &mut ph);
            agc.process(&mut f, true);
        }
        assert!(
            (agc.gain_db() - before).abs() < 1.0,
            "{:.1} dB, was {before:.1} dB",
            agc.gain_db()
        );
    }

    #[test]
    fn reset_returns_to_unity() {
        let mut agc = Agc::new();
        let mut ph = 0.0;
        for _ in 0..100 {
            let mut f = tone(-32.0, &mut ph);
            agc.process(&mut f, true);
        }
        assert!(agc.gain_db() > 5.0);
        agc.reset();
        assert_eq!(agc.gain_db(), 0.0);
        let mut f = tone(-32.0, &mut ph);
        let before = f.clone();
        agc.process(&mut f, false);
        assert_eq!(f, before, "unity gain leaves the frame untouched");
    }

    #[test]
    fn gain_changes_are_smooth_within_a_frame() {
        let mut agc = Agc::new();
        // A constant (DC) input makes any step in the gain visible as a step in the output.
        let mut f = vec![0.01f32; FRAME];
        for _ in 0..10 {
            f.fill(0.01);
            agc.process(&mut f, true);
        }
        let max_step = f
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0f32, f32::max);
        assert!(max_step < 1e-4, "step {max_step}");
    }
}
