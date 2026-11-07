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

/// A gap this long eats half of the 40 ms output reserve.
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
