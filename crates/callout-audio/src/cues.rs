//! Join/leave cues: short sounds played locally when someone enters or leaves the room.
//!
//! Synthesized, not sampled: no sound files to license or ship. Every style has a rising "join"
//! and a falling "leave" form. Output is 48 kHz mono, a whole number of 20 ms frames, peaking at
//! [`PEAK`], with a fade at both ends so it never clicks. Rendered outside the cpal callbacks
//! (the engine's audio thread mixes it like the test sound).

use crate::codec::FRAME;
use callout_proto::SAMPLE_RATE_HZ;
use std::f32::consts::TAU;

/// Peak level of every cue: noticeable over voices without startling (the test sound is 0.25).
pub const PEAK: f32 = 0.22;

/// The sound family; picked by the player in settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Two soft bell notes.
    Chime,
    /// A single bubble "bloop" (fast pitch glide).
    Pop,
    /// Two wooden mallet notes.
    Marimba,
    /// Two short radio beeps.
    Beep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    Join,
    Leave,
}

const SR: f32 = SAMPLE_RATE_HZ as f32;

/// Renders a cue (allocates; call outside the audio callbacks).
pub fn render(style: Style, cue: Cue) -> Vec<f32> {
    let rising = cue == Cue::Join;
    let mut out = match style {
        Style::Chime => notes(0.5, 0.09, [784.0, 1174.66], rising, |hz, dt| {
            // Sine with a quieter octave: bell-like, decays in ~0.4 s.
            let tone = (TAU * hz * dt).sin() + 0.25 * (TAU * 2.0 * hz * dt).sin();
            tone * envelope(dt, 0.004, 7.0)
        }),
        Style::Marimba => notes(0.45, 0.07, [659.25, 987.77], rising, |hz, dt| {
            // A marimba bar's strong overtone sits near 4× the fundamental and dies fast.
            let tone =
                (TAU * hz * dt).sin() + 0.35 * (TAU * 4.0 * hz * dt).sin() * (-dt * 40.0).exp();
            tone * envelope(dt, 0.002, 12.0)
        }),
        Style::Beep => notes(0.2, 0.09, [880.0, 1320.0], rising, |hz, dt| {
            if dt > 0.06 {
                return 0.0;
            }
            // Band-limited square (odd harmonics 1, 3, 5) gated to 60 ms with 4 ms ramps.
            let w = TAU * hz * dt;
            let tone = w.sin() + (3.0 * w).sin() / 3.0 + (5.0 * w).sin() / 5.0;
            let gate = (dt / 0.004).min(1.0) * ((0.06 - dt) / 0.004).min(1.0);
            tone * gate
        }),
        Style::Pop => pop(rising),
    };
    finish(&mut out);
    out
}

/// Two notes `gap` seconds apart (low then high when rising), each shaped by `voice(hz, dt)`.
fn notes(
    secs: f32,
    gap: f32,
    [low, high]: [f32; 2],
    rising: bool,
    voice: impl Fn(f32, f32) -> f32,
) -> Vec<f32> {
    let (first, second) = if rising { (low, high) } else { (high, low) };
    (0..(secs * SR) as usize)
        .map(|i| {
            let t = i as f32 / SR;
            let mut s = voice(first, t);
            if t >= gap {
                s += voice(second, t - gap);
            }
            s
        })
        .collect()
}

/// A bubble: a sine whose pitch glides exponentially (up for join, down for leave) in 70 ms.
fn pop(rising: bool) -> Vec<f32> {
    let (from, to) = if rising {
        (400.0f32, 900.0f32)
    } else {
        (900.0, 400.0)
    };
    let glide = 0.07;
    let mut phase = 0.0f32;
    (0..(0.16 * SR) as usize)
        .map(|i| {
            let t = i as f32 / SR;
            let hz = from * (to / from).powf((t / glide).min(1.0));
            phase = (phase + TAU * hz / SR) % TAU;
            phase.sin() * envelope(t, 0.003, 30.0)
        })
        .collect()
}

/// Linear attack, exponential decay.
fn envelope(dt: f32, attack: f32, decay: f32) -> f32 {
    (dt / attack).min(1.0) * (-dt * decay).exp()
}

/// Normalizes to [`PEAK`], fades the last 10 ms out and pads to whole frames.
fn finish(out: &mut Vec<f32>) {
    let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if peak > 0.0 {
        out.iter_mut().for_each(|s| *s *= PEAK / peak);
    }
    let fade = ((0.01 * SR) as usize).min(out.len());
    let n = out.len();
    for (k, s) in out[n - fade..].iter_mut().enumerate() {
        *s *= 1.0 - (k + 1) as f32 / fade as f32;
    }
    out.resize(n.div_ceil(FRAME) * FRAME, 0.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Style; 4] = [Style::Chime, Style::Pop, Style::Marimba, Style::Beep];

    #[test]
    fn cues_are_frame_aligned_short_and_at_the_peak() {
        for style in ALL {
            for cue in [Cue::Join, Cue::Leave] {
                let s = render(style, cue);
                assert_eq!(s.len() % FRAME, 0, "{style:?} {cue:?}");
                assert!(s.len() <= (0.5 * SR) as usize + FRAME, "{style:?} too long");
                let peak = s.iter().fold(0.0f32, |m, x| m.max(x.abs()));
                assert!((peak - PEAK).abs() < 1e-3, "{style:?} {cue:?} peak {peak}");
                assert!(s.iter().all(|x| x.is_finite()));
            }
        }
    }

    #[test]
    fn cues_start_and_end_silent() {
        // No click: the first sample is near zero and the tail is faded out.
        for style in ALL {
            for cue in [Cue::Join, Cue::Leave] {
                let s = render(style, cue);
                assert!(s[0].abs() < 0.02, "{style:?} {cue:?} starts at {}", s[0]);
                assert_eq!(*s.last().unwrap(), 0.0);
            }
        }
    }

    #[test]
    fn join_and_leave_differ() {
        for style in ALL {
            assert_ne!(
                render(style, Cue::Join),
                render(style, Cue::Leave),
                "{style:?}"
            );
        }
    }
}
