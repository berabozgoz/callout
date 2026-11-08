//! Echo simulation: what we play leaks into the microphone through a delayed, reverberant path
//! (speakers, or loud headphones next to the mic); the canceller has to remove it while keeping
//! the player's own voice.
//!
//! Signals are synthetic and deterministic: the far end (the others in the room) and the near end
//! (the player) are speech-like with different pitch and rhythm, plus a little mic noise.
//! To see the numbers: `cargo test -p callout-audio --test echo_sim -- --nocapture`
//! (in release for the speed figure: add `--release`).

use callout_audio::aec::EchoCanceller;
use callout_audio::codec::FRAME;
use std::f32::consts::TAU;
use std::time::Instant;

const SR: f32 = 48_000.0;

/// Deterministic noise in [-1, 1] (LCG).
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }
}

/// Speech-like signal: gliding pitch with harmonics, a breathy noise part, syllables at
/// `rate` Hz and pauses every few seconds.
fn voice(samples: usize, f0: f32, rate: f32, level: f32, seed: u64) -> Vec<f32> {
    let mut noise = Noise(seed);
    let mut phase = 0f32;
    let mut lp = 0f32;
    (0..samples)
        .map(|i| {
            let t = i as f32 / SR;
            let pitch = f0 * (1.0 + 0.2 * (TAU * 0.5 * t + seed as f32).sin());
            phase = (phase + TAU * pitch / SR) % TAU;
            let mut s = 0.0;
            for h in 1..=12 {
                s += (phase * h as f32).sin() / h as f32;
            }
            lp += 0.3 * (noise.next() - lp);
            let syllable = (0.5 + 0.5 * (TAU * rate * t).sin()).powi(2);
            let talking = if (t * 0.4 + seed as f32 * 0.1).fract() < 0.75 {
                1.0
            } else {
                0.0
            };
            level * syllable * talking * (0.6 * s + 0.4 * lp)
        })
        .collect()
}

/// Echo path: direct sound after `delay_ms` plus decaying reflections, scaled by `gain_db`.
fn echo(far: &[f32], delay_ms: f32, gain_db: f32) -> Vec<f32> {
    let g = 10f32.powf(gain_db / 20.0);
    let d = (delay_ms * SR / 1000.0) as usize;
    // (extra delay in ms, relative level)
    let taps = [
        (0.0, 1.0),
        (3.1, 0.5),
        (7.4, -0.35),
        (13.0, 0.2),
        (21.0, -0.12),
        (34.0, 0.06),
    ];
    let mut out = vec![0f32; far.len()];
    for (extra, a) in taps {
        let off = d + (extra * SR / 1000.0) as usize;
        for i in off..far.len() {
            out[i] += g * a * far[i - off];
        }
    }
    out
}

fn power(x: &[f32]) -> f64 {
    x.iter().map(|&s| (s as f64) * (s as f64)).sum::<f64>() / x.len().max(1) as f64
}

fn db(p: f64) -> f64 {
    10.0 * p.max(1e-20).log10()
}

struct Run {
    out: Vec<f32>,
    secs_per_sec: f64,
}

/// Plays `far`, hears `mic` (near + echo + noise), returns the cleaned microphone signal.
fn run(far: &[f32], mic: &[f32], delay_hint_ms: u32) -> Run {
    let mut aec = EchoCanceller::new();
    aec.set_delay_ms(delay_hint_ms);
    let mut out = vec![0f32; mic.len()];
    let started = Instant::now();
    for (k, o) in out.as_chunks_mut::<FRAME>().0.iter_mut().enumerate() {
        let r = k * FRAME..(k + 1) * FRAME;
        aec.render(&far[r.clone()]);
        o.copy_from_slice(&mic[r]);
        aec.capture(o);
    }
    let secs = mic.len() as f64 / SR as f64;
    Run {
        out,
        secs_per_sec: started.elapsed().as_secs_f64() / secs,
    }
}

fn mic_noise(samples: usize) -> Vec<f32> {
    let mut n = Noise(99);
    (0..samples).map(|_| 0.0005 * n.next()).collect() // ≈ −70 dBFS
}

/// Segment of `x` in seconds.
fn seg(x: &[f32], from: f32, to: f32) -> &[f32] {
    &x[(from * SR) as usize..(to * SR) as usize]
}

#[test]
fn removes_speaker_echo_when_only_the_others_talk() {
    let secs = 20.0;
    let n = (secs * SR) as usize / FRAME * FRAME;
    let far = voice(n, 140.0, 3.5, 0.3, 1);
    let echo_sig = echo(&far, 60.0, -12.0); // speakers on a desk
    let noise = mic_noise(n);
    let mic: Vec<f32> = echo_sig.iter().zip(&noise).map(|(e, z)| e + z).collect();
    let r = run(&far, &mic, 60);
    // After 5 s of learning.
    let before = power(seg(&mic, 5.0, secs));
    let after = power(seg(&r.out, 5.0, secs));
    let reduction = db(before) - db(after);
    println!(
        "speakers: echo {:.1} dBFS → {:.1} dBFS ({reduction:.1} dB removed); {:.3} s CPU per s audio",
        db(before),
        db(after),
        r.secs_per_sec
    );
    assert!(reduction > 25.0, "only {reduction:.1} dB of echo removed");
}

#[test]
fn removes_quiet_headphone_leak() {
    let secs = 20.0;
    let n = (secs * SR) as usize / FRAME * FRAME;
    let far = voice(n, 150.0, 3.0, 0.3, 2);
    let echo_sig = echo(&far, 45.0, -32.0); // loud headphones next to a webcam mic
    let noise = mic_noise(n);
    let mic: Vec<f32> = echo_sig.iter().zip(&noise).map(|(e, z)| e + z).collect();
    let r = run(&far, &mic, 45);
    let echo_db = db(power(seg(&echo_sig, 5.0, secs)));
    let after_db = db(power(seg(&r.out, 5.0, secs)));
    let noise_db = db(power(seg(&noise, 5.0, secs)));
    println!("headphone leak: echo {echo_db:.1} dBFS → output {after_db:.1} dBFS (mic noise {noise_db:.1})");
    // The leak must end up near the noise floor (an echo 3 dB above it is hardly audible
    // after noise suppression and the voice gate).
    assert!(after_db < noise_db + 3.0, "leak left at {after_db:.1} dBFS");
}

#[test]
fn keeps_the_players_voice() {
    // With both sides talking most of the time the canceller needs ~20–30 s to learn the echo
    // path (a few seconds when the others talk alone); measured after 25 s.
    let secs = 50.0;
    let learn = 25.0;
    let n = (secs * SR) as usize / FRAME * FRAME;
    let far = voice(n, 140.0, 3.5, 0.3, 1);
    let near = voice(n, 220.0, 4.5, 0.2, 7);
    let echo_sig = echo(&far, 60.0, -12.0);
    let noise = mic_noise(n);
    let mic: Vec<f32> = (0..n).map(|i| near[i] + echo_sig[i] + noise[i]).collect();
    let r = run(&far, &mic, 60);

    // Without the far end (headset, nobody else talking) the voice passes untouched.
    let silent = vec![0f32; n];
    let alone: Vec<f32> = (0..n).map(|i| near[i] + noise[i]).collect();
    let r_alone = run(&silent, &alone, 60);
    let kept_alone = db(power(seg(&r_alone.out, 5.0, secs))) - db(power(seg(&alone, 5.0, secs)));

    // Frames (after learning) by who is talking, from the clean signals: "talking"
    // above −45 dBFS, "silent" only in a real pause (nothing at all), so a quiet syllable of
    // one side does not count as the other side's echo or voice.
    let level = |x: &[f32]| db(power(x));
    let (mut far_only, mut near_only, mut both) = ([0f64; 2], [0f64; 2], [0f64; 2]);
    for k in (learn * SR) as usize / FRAME..n / FRAME {
        let f = k * FRAME..(k + 1) * FRAME;
        let (e, v) = (level(&echo_sig[f.clone()]), level(&near[f.clone()]));
        // [input power, output power]; for voice frames the input is the clean voice.
        let acc = if e > -45.0 && v < -100.0 {
            (&mut far_only, power(&mic[f.clone()]))
        } else if e < -100.0 && v > -45.0 {
            (&mut near_only, power(&near[f.clone()]))
        } else if e > -45.0 && v > -45.0 {
            (&mut both, power(&near[f.clone()]))
        } else {
            continue;
        };
        acc.0[0] += acc.1;
        acc.0[1] += power(&r.out[f]);
    }
    let change = |a: [f64; 2]| db(a[1]) - db(a[0]);
    println!(
        "voice alone {kept_alone:+.1} dB; with echo: others only {:+.1} dB (echo removed), player only {:+.1} dB, both talking {:+.1} dB (vs the voice)",
        change(far_only),
        change(near_only),
        change(both)
    );
    assert!(
        kept_alone.abs() < 1.5,
        "voice alone changed by {kept_alone:+.1} dB"
    );
    assert!(
        change(far_only) < -20.0,
        "echo not removed while others talk"
    );
    assert!(
        change(near_only).abs() < 2.0,
        "voice changed while only the player talks"
    );
    assert!(change(both) > -6.0, "voice suppressed during double talk");
}
