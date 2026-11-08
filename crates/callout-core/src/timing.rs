//! Scheduling diagnostics for the audio path: how late the audio callbacks and the engine's audio
//! thread ran. Under a game's CPU load these tell a starved thread apart from network jitter.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Largest gap between two calls of an audio callback since the last [`GapMeter::take_max`].
/// [`GapMeter::tick`] uses atomics only, so it is safe in a cpal callback (CLAUDE.md rule 1).
pub(crate) struct GapMeter {
    origin: Instant,
    /// Time of the previous tick in µs since `origin`; 0 = no tick yet.
    last_us: AtomicU64,
    max_us: AtomicU32,
}

impl GapMeter {
    pub(crate) fn new() -> Self {
        Self {
            origin: Instant::now(),
            last_us: AtomicU64::new(0),
            max_us: AtomicU32::new(0),
        }
    }

    /// Called at the start of every callback.
    pub(crate) fn tick(&self) {
        self.tick_at(self.origin.elapsed());
    }

    fn tick_at(&self, since_origin: Duration) {
        let now = (since_origin.as_micros() as u64).max(1);
        let prev = self.last_us.swap(now, Ordering::Relaxed);
        if prev != 0 {
            let gap = now.saturating_sub(prev).min(u32::MAX as u64) as u32;
            self.max_us.fetch_max(gap, Ordering::Relaxed);
        }
    }

    /// The largest gap since the previous call (then starts over); `None` before two ticks.
    pub(crate) fn take_max(&self) -> Option<Duration> {
        let max = self.max_us.swap(0, Ordering::Relaxed);
        (max > 0).then(|| Duration::from_micros(max.into()))
    }
}

/// The engine audio loop's own pacing: largest gap between iterations and how many gaps were
/// long enough to risk a playback underrun.
pub(crate) struct LoopMeter {
    last: Instant,
    max: Duration,
    stalls: u32,
}

/// A gap this long empties the smallest output reserve ([`RESERVE_MIN_MS`]).
pub(crate) const LOOP_STALL: Duration = Duration::from_millis(20);

impl LoopMeter {
    pub(crate) fn new() -> Self {
        Self {
            last: Instant::now(),
            max: Duration::ZERO,
            stalls: 0,
        }
    }

    /// Called once per loop iteration.
    pub(crate) fn tick(&mut self) {
        self.tick_at(Instant::now());
    }

    fn tick_at(&mut self, now: Instant) {
        let gap = now.saturating_duration_since(self.last);
        self.last = now;
        self.max = self.max.max(gap);
        if gap >= LOOP_STALL {
            self.stalls += 1;
        }
    }

    /// `(largest gap, stalls)` since the previous call; then starts over.
    pub(crate) fn take(&mut self) -> (Duration, u32) {
        let r = (self.max, self.stalls);
        self.max = Duration::ZERO;
        self.stalls = 0;
        r
    }
}

pub(crate) fn ms(d: Option<Duration>) -> f32 {
    d.map_or(0.0, |d| d.as_secs_f32() * 1000.0)
}

/// The latest device buffering reported by a cpal callback (input: capture → callback; output:
/// callback → estimated playback). [`DeviceLatency::set`] uses one atomic store, so it is safe in
/// a cpal callback (CLAUDE.md rule 1). cpal's WASAPI output figure is an estimate (the writable
/// part of the device buffer), good enough to tell a 10 ms device from a 40 ms one.
pub(crate) struct DeviceLatency {
    /// µs + 1; 0 = no report yet.
    us: AtomicU32,
}

impl DeviceLatency {
    pub(crate) fn new() -> Self {
        Self {
            us: AtomicU32::new(0),
        }
    }

    pub(crate) fn set(&self, d: Option<Duration>) {
        if let Some(d) = d {
            let us = d.as_micros().min(u32::MAX as u128 - 1) as u32;
            self.us.store(us + 1, Ordering::Relaxed);
        }
    }

    pub(crate) fn get(&self) -> Option<Duration> {
        match self.us.load(Ordering::Relaxed) {
            0 => None,
            us => Some(Duration::from_micros((us - 1).into())),
        }
    }
}

/// Output reserve at the start of a pipeline (ms of audio queued ahead of the playback
/// callback). It used to be a fixed 40 ms (~50 ms average queue); with the audio thread's raised
/// priority a smaller reserve is usually enough, and it grows on its own where it is not.
pub(crate) const RESERVE_START_MS: u32 = 30;
/// Smallest reserve: two 10 ms device periods.
pub(crate) const RESERVE_MIN_MS: u32 = 20;
pub(crate) const RESERVE_MAX_MS: u32 = 80;
/// Step in both directions. The device takes audio in ~10 ms periods, so the ring's fill moves in
/// 10 ms steps: a smaller change of the threshold often changes nothing (seen in a local run).
const RESERVE_STEP_MS: u32 = 10;
/// Underrun-free time before the reserve tries a smaller value.
const RESERVE_QUIET: Duration = Duration::from_secs(120);
/// An underrun this soon after lowering means the lower value was too small: it becomes off
/// limits for the rest of the pipeline's life, so a busy PC clicks at most once per probe level.
const RESERVE_PROBATION: Duration = Duration::from_secs(60);

/// Adapts the output reserve: grows at once when the playback callback ran dry (each underrun
/// is an audible click), shrinks slowly while it does not.
pub(crate) struct OutputReserve {
    ms: u32,
    /// Lowest value allowed; raised when a lowered value failed.
    floor: u32,
    quiet: Duration,
    /// Time since the last lowering and the value before it.
    probation: Option<(Duration, u32)>,
}

impl OutputReserve {
    pub(crate) fn new() -> Self {
        Self {
            ms: RESERVE_START_MS,
            floor: RESERVE_MIN_MS,
            quiet: Duration::ZERO,
            probation: None,
        }
    }

    pub(crate) fn ms(&self) -> u32 {
        self.ms
    }

    /// Called periodically with the time since the previous call and whether the playback
    /// callback ran dry meanwhile. Returns the new value when it changed.
    pub(crate) fn update(&mut self, elapsed: Duration, underrun: bool) -> Option<u32> {
        let before = self.ms;
        if underrun {
            if let Some((since, previous)) = self.probation.take() {
                if since + elapsed <= RESERVE_PROBATION {
                    self.floor = self.floor.max(previous);
                }
            }
            self.ms = (self.ms + RESERVE_STEP_MS)
                .max(self.floor)
                .min(RESERVE_MAX_MS);
            self.quiet = Duration::ZERO;
        } else {
            if let Some((since, _)) = &mut self.probation {
                *since += elapsed;
            }
            self.quiet += elapsed;
            if self.quiet >= RESERVE_QUIET && self.ms > self.floor {
                self.probation = Some((Duration::ZERO, self.ms));
                self.ms = self.ms.saturating_sub(RESERVE_STEP_MS).max(self.floor);
                self.quiet = Duration::ZERO;
            }
        }
        (self.ms != before).then_some(self.ms)
    }
}

/// Average fill of the output ring (time-weighted by loop iterations, which run every ~2 ms).
pub(crate) struct FillMeter {
    sum: u64,
    n: u64,
}

impl FillMeter {
    pub(crate) fn new() -> Self {
        Self { sum: 0, n: 0 }
    }

    pub(crate) fn sample(&mut self, samples: usize) {
        self.sum += samples as u64;
        self.n += 1;
    }

    /// Average in ms at `rate` since the previous call; then starts over.
    pub(crate) fn take_ms(&mut self, rate: u32) -> Option<f32> {
        let r = (self.n > 0).then(|| self.sum as f32 / self.n as f32 * 1000.0 / rate as f32);
        self.sum = 0;
        self.n = 0;
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gap_meter_keeps_the_largest_gap_and_resets() {
        let m = GapMeter::new();
        assert_eq!(m.take_max(), None);
        m.tick_at(Duration::from_millis(100));
        assert_eq!(m.take_max(), None, "one tick has no gap");
        m.tick_at(Duration::from_millis(110));
        m.tick_at(Duration::from_millis(155));
        m.tick_at(Duration::from_millis(165));
        assert_eq!(m.take_max(), Some(Duration::from_millis(45)));
        assert_eq!(m.take_max(), None);
        // The gap across a take still counts in the next window.
        m.tick_at(Duration::from_millis(175));
        assert_eq!(m.take_max(), Some(Duration::from_millis(10)));
    }

    #[test]
    fn gap_meter_first_tick_at_zero_is_not_lost() {
        let m = GapMeter::new();
        m.tick_at(Duration::ZERO);
        m.tick_at(Duration::from_millis(10));
        assert!(m.take_max().is_some());
    }

    const SEC: Duration = Duration::from_secs(1);

    /// Runs `secs` one-second windows without underruns; returns the changes.
    fn quiet(r: &mut OutputReserve, secs: u32) -> Vec<u32> {
        (0..secs).filter_map(|_| r.update(SEC, false)).collect()
    }

    #[test]
    fn reserve_shrinks_slowly_to_the_minimum_when_quiet() {
        let mut r = OutputReserve::new();
        assert_eq!(r.ms(), RESERVE_START_MS);
        assert_eq!(
            quiet(&mut r, 119),
            Vec::<u32>::new(),
            "nothing before the quiet period"
        );
        assert_eq!(quiet(&mut r, 1), vec![RESERVE_MIN_MS]);
        assert_eq!(
            quiet(&mut r, 600),
            Vec::<u32>::new(),
            "never below the minimum"
        );
    }

    #[test]
    fn reserve_grows_at_once_on_underrun_up_to_the_maximum() {
        let mut r = OutputReserve::new();
        assert_eq!(r.update(SEC, true), Some(RESERVE_START_MS + 10));
        for _ in 0..10 {
            r.update(SEC, true);
        }
        assert_eq!(r.ms(), RESERVE_MAX_MS);
        assert_eq!(r.update(SEC, true), None);
    }

    #[test]
    fn reserve_does_not_retry_a_value_that_failed() {
        let mut r = OutputReserve::new();
        quiet(&mut r, 120); // 30 → 20
        quiet(&mut r, 30);
        assert_eq!(r.update(SEC, true), Some(30), "20 failed within probation");
        // 30 holds, and 20 is not tried again.
        assert_eq!(quiet(&mut r, 600), Vec::<u32>::new());
        // A later underrun still raises it.
        assert_eq!(r.update(SEC, true), Some(40));
        assert_eq!(quiet(&mut r, 600), vec![30]);
    }

    #[test]
    fn reserve_underrun_after_probation_does_not_raise_the_floor() {
        let mut r = OutputReserve::new();
        quiet(&mut r, 120); // 30 → 20
        quiet(&mut r, 90); // 20 held past probation: it works
        assert_eq!(
            r.update(SEC, true),
            Some(30),
            "a one-off (e.g. a game starting)"
        );
        assert_eq!(quiet(&mut r, 120), vec![20], "20 may be tried again");
    }

    #[test]
    fn fill_meter_averages_in_ms() {
        let mut f = FillMeter::new();
        assert_eq!(f.take_ms(48_000), None);
        f.sample(960); // 20 ms
        f.sample(1920); // 40 ms
        assert_eq!(f.take_ms(48_000), Some(30.0));
        assert_eq!(f.take_ms(48_000), None);
    }

    #[test]
    fn device_latency_keeps_the_latest_report() {
        let d = DeviceLatency::new();
        assert_eq!(d.get(), None);
        d.set(Some(Duration::ZERO));
        assert_eq!(d.get(), Some(Duration::ZERO));
        d.set(Some(Duration::from_millis(10)));
        d.set(None); // no estimate this time: keep the last one
        assert_eq!(d.get(), Some(Duration::from_millis(10)));
    }

    #[test]
    fn loop_meter_counts_stalls() {
        let t0 = Instant::now();
        let mut m = LoopMeter::new();
        m.last = t0;
        let mut at = 0;
        for gap_ms in [2, 3, 25, 2, 60, 2] {
            at += gap_ms;
            m.tick_at(t0 + Duration::from_millis(at));
        }
        assert_eq!(m.take(), (Duration::from_millis(60), 2));
        assert_eq!(m.take(), (Duration::ZERO, 0));
    }
}
