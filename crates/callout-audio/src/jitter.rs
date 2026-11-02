//! Per-sender jitter buffer.
//!
//! Packets are ordered by `seq`; [`JitterBuffer::pop`] is called on every 20 ms
//! playout tick and returns one of three outcomes: a packet, a loss (fill with
//! PLC/FEC) or idle (the sender went quiet, silence). The target depth (`target`)
//! is the prefill duration: after the first packet, playout does not start until
//! the buffer reaches `target` frames.
//!
//! The target follows the measured delay variation ([`DelayEstimator`]): every packet's
//! arrival time is compared with its sender timestamp, and the target covers the spread
//! up to the `DELAY_QUANTILE` of the last ~20 s. A few late packets (concealed by PLC/FEC)
//! are accepted instead of growing the buffer on every burst, and the target shrinks as
//! soon as the network calms down. Without timing data (`push`), a late/underflow
//! counter heuristic is used instead.
//!
//! Fixed capacity with few allocations: packets are stored as `Vec<u8>`; this
//! module runs on the audio thread (not in the real-time callback).

use std::collections::{BTreeMap, VecDeque};

/// Share of packets the target should cover in time; the rest arrive late and are concealed
/// (FEC when the next packet is there, else PLC). Chosen with `tests/jitter_sim.rs`: 0.95 cuts
/// Wi-Fi buffering ~30% vs. the old heuristic for ~1.5% concealed frames; 0.97–0.98 buys little.
const DELAY_QUANTILE: f64 = 0.95;
/// Delay samples kept (20 ms frames → ~20 s of speech).
const DELAY_WINDOW: usize = 1000;
/// Samples needed before the estimate replaces the heuristic (~2 s of speech).
const DELAY_MIN_SAMPLES: usize = 100;
/// Packets right after TALK_START are skipped: the sender's pre-roll frames go out in a burst
/// and would look delayed.
const SKIP_AFTER_TALK_START: u32 = 4;
/// A jump this large (ms) means the sender's clock restarted (new audio pipeline): start over.
const DELAY_RESET_MS: f64 = 1000.0;

/// Tracks how much later than "on time" packets arrive, relative to the best case.
///
/// Relative delay = arrival time − sender timestamp (both in ms). Its minimum over the window is
/// the network's base delay; how far the `DELAY_QUANTILE` sits above it is what the buffer must
/// absorb. The sender's timestamp keeps counting through VAD pauses and DTX, so gaps do not skew it.
pub struct DelayEstimator {
    samples: VecDeque<f64>,
    scratch: Vec<f64>,
    last_ts: Option<u32>,
    ts_ms: f64,
    skip: u32,
}

impl Default for DelayEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl DelayEstimator {
    pub fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(DELAY_WINDOW),
            scratch: Vec::with_capacity(DELAY_WINDOW),
            last_ts: None,
            ts_ms: 0.0,
            skip: 0,
        }
    }

    /// `timestamp`: sender sample counter (48 kHz); `arrival_ms`: local arrival time.
    pub fn observe(&mut self, timestamp: u32, arrival_ms: f64, talk_start: bool) {
        match self.last_ts {
            // Unwrap the 32-bit counter (wraps every ~25 h) via signed differences.
            Some(last) => self.ts_ms += timestamp.wrapping_sub(last) as i32 as f64 / 48.0,
            None => self.ts_ms = timestamp as f64 / 48.0,
        }
        self.last_ts = Some(timestamp);
        if talk_start {
            self.skip = SKIP_AFTER_TALK_START;
        }
        if self.skip > 0 {
            self.skip -= 1;
            return;
        }
        let d = arrival_ms - self.ts_ms;
        if let Some(&last) = self.samples.back() {
            if (d - last).abs() > DELAY_RESET_MS {
                self.samples.clear();
            }
        }
        if self.samples.len() == DELAY_WINDOW {
            self.samples.pop_front();
        }
        self.samples.push_back(d);
    }

    /// Delay spread (ms) the buffer has to absorb, or `None` with too little data.
    pub fn spread_ms(&mut self) -> Option<f64> {
        if self.samples.len() < DELAY_MIN_SAMPLES {
            return None;
        }
        self.scratch.clear();
        self.scratch.extend(self.samples.iter().copied());
        let min = self.scratch.iter().copied().fold(f64::INFINITY, f64::min);
        let k = ((self.scratch.len() - 1) as f64 * DELAY_QUANTILE).round() as usize;
        let (_, q, _) = self
            .scratch
            .select_nth_unstable_by(k, |a, b| a.total_cmp(b));
        Some(*q - min)
    }

    /// Target depth in frames for the current spread: playout starts when this many frames are
    /// queued, so `target − 1` frames of slack cover the spread.
    pub fn target_frames(&mut self) -> Option<u32> {
        self.spread_ms().map(|s| (s / 20.0).ceil() as u32 + 1)
    }
}

/// The buffer's decision on a playout tick.
#[derive(Debug, PartialEq, Eq)]
pub enum Playout {
    /// The next packet; Opus payload.
    Packet(Vec<u8>),
    /// The next packet is lost. If `next` is set, the following packet is at hand and
    /// the lost frame can be recovered with `decode_fec`; otherwise PLC (`decode(None)`).
    Lost { next: Option<Vec<u8>> },
    /// The sender went quiet or prefill is not yet complete; produce silence.
    Idle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JitterStats {
    pub late: u32,
    pub lost: u32,
    pub fec_recovered: u32,
    pub underflow: u32,
    pub target: u32,
}

pub struct JitterBuffer {
    packets: BTreeMap<u64, Vec<u8>>,
    /// Unwrapped sequence counter; `None` = idle.
    next: Option<u64>,
    last_unwrapped: Option<u64>,
    prefilling: bool,
    /// The current prefill is a resync after a network stall (the stream went idle without the
    /// sender stopping): the backlog that arrives in one clump is stale and is trimmed to
    /// `target` instead of being played late for the rest of the talk spurt.
    resync: bool,
    misses: u32,
    /// Ticks spent with an empty buffer; counted as underflow once it is clear the
    /// stream was really delayed (it continues in sequence, without TALK_START).
    pending_misses: u32,
    target: u32,
    min_target: u32,
    max_target: u32,
    window_late: u32,
    window_underflow: u32,
    quiet_windows: u32,
    delay: DelayEstimator,
    stats: JitterStats,
}

/// With no packet for this many consecutive ticks the sender is considered quiet (PLC stops).
const IDLE_AFTER_MISSES: u32 = 3;
/// If the buffer exceeds this many frames (target + margin), the oldest are dropped.
const OVERFLOW_MARGIN: u32 = 8;

impl JitterBuffer {
    /// `target_frames`: initial prefill (in 20 ms units; 3 = 60 ms).
    pub fn new(target_frames: u32) -> Self {
        let target = target_frames.clamp(1, 20);
        Self {
            packets: BTreeMap::new(),
            next: None,
            last_unwrapped: None,
            prefilling: true,
            resync: false,
            misses: 0,
            pending_misses: 0,
            target,
            min_target: 2,
            max_target: 12,
            window_late: 0,
            window_underflow: 0,
            quiet_windows: 0,
            delay: DelayEstimator::new(),
            stats: JitterStats {
                target,
                ..Default::default()
            },
        }
    }

    fn unwrap_seq(&mut self, seq: u16) -> u64 {
        let Some(last) = self.last_unwrapped else {
            self.last_unwrapped = Some(seq as u64 + 65536); // headroom for negative differences
            return self.last_unwrapped.unwrap();
        };
        let last_lo = (last & 0xFFFF) as u16;
        let diff = seq.wrapping_sub(last_lo) as i16 as i64;
        let unwrapped = (last as i64 + diff).max(0) as u64;
        self.last_unwrapped = Some(unwrapped);
        unwrapped
    }

    /// A packet from the network. `talk_start` (the TALK_START flag) marks the start
    /// of a new talk spurt; the buffer is reset and prefilled again.
    pub fn push(&mut self, seq: u16, talk_start: bool, opus: &[u8]) {
        if talk_start || self.next.is_none() {
            // New talk spurt: the previous gap was the sender going quiet, no penalty. Without
            // TALK_START the sender never stopped: we went idle because the network stalled.
            self.resync = !talk_start;
            self.packets.clear();
            self.last_unwrapped = None;
            let u = self.unwrap_seq(seq);
            self.next = Some(u);
            self.prefilling = true;
            self.misses = 0;
            self.pending_misses = 0;
            self.packets.insert(u, opus.to_vec());
            return;
        }
        let u = self.unwrap_seq(seq);
        let next = self.next.unwrap();
        if self.pending_misses > 0 {
            if u <= next {
                // The stream was continuous and we were waiting: real delay.
                self.stats.underflow += self.pending_misses;
                self.window_underflow += self.pending_misses;
            }
            self.pending_misses = 0;
        }
        if u < next {
            // Its playout slot has passed: late packet.
            self.stats.late += 1;
            self.window_late += 1;
            return;
        }
        self.packets.insert(u, opus.to_vec());
        // Overflow: if far above the target, drop the oldest (bounds latency).
        let limit = (self.target + OVERFLOW_MARGIN) as usize;
        while self.packets.len() > limit {
            if let Some((&oldest, _)) = self.packets.iter().next() {
                self.packets.remove(&oldest);
                self.next = Some(oldest + 1);
            }
        }
    }

    /// [`JitterBuffer::push`] with timing: `timestamp` is the packet's sender sample counter,
    /// `arrival_ms` the local receive time (any monotonic origin). Feeds the delay estimate.
    pub fn push_timed(
        &mut self,
        seq: u16,
        timestamp: u32,
        talk_start: bool,
        opus: &[u8],
        arrival_ms: f64,
    ) {
        self.delay.observe(timestamp, arrival_ms, talk_start);
        self.push(seq, talk_start, opus)
    }

    /// Called on every 20 ms playout tick.
    pub fn pop(&mut self) -> Playout {
        if self.next.is_none() {
            return Playout::Idle;
        }

        if self.prefilling {
            if (self.packets.len() as u32) < self.target {
                return Playout::Idle;
            }
            self.prefilling = false;
            if self.resync {
                // Catch up after a stall: skip the stale part of the clump.
                while self.packets.len() as u32 > self.target {
                    let (&oldest, _) = self.packets.iter().next().unwrap();
                    self.packets.remove(&oldest);
                    self.stats.lost += 1;
                }
                self.next = self.packets.keys().next().copied();
                self.resync = false;
            }
        }
        let Some(next) = self.next else {
            return Playout::Idle;
        };

        if let Some(p) = self.packets.remove(&next) {
            self.next = Some(next + 1);
            self.misses = 0;
            return Playout::Packet(p);
        }

        // The next one is missing.
        if self.packets.is_empty() {
            self.misses += 1;
            if self.misses >= IDLE_AFTER_MISSES {
                // Talk spurt over: idle. The next packet starts a new prefill.
                self.next = None;
                self.misses = 0;
                self.pending_misses = 0;
                return Playout::Idle;
            }
            // Not yet known whether it is the network or silence; the next push decides.
            self.pending_misses += 1;
            self.next = Some(next + 1);
            return Playout::Lost { next: None };
        }

        // A later packet exists: this frame is really lost.
        self.stats.lost += 1;
        self.next = Some(next + 1);
        let fec_src = self.packets.get(&(next + 1)).cloned();
        if fec_src.is_some() {
            self.stats.fec_recovered += 1;
        }
        Playout::Lost { next: fec_src }
    }

    /// Called every 5 seconds: adjusts the target depth to the measured delay spread, or (without
    /// timing data) based on the window's late/underflow counts.
    pub fn adapt(&mut self) {
        if let Some(t) = self.delay.target_frames() {
            self.target = t.clamp(self.min_target, self.max_target);
            self.quiet_windows = 0;
        } else if self.window_late + self.window_underflow >= 3 {
            self.target = (self.target + 1).min(self.max_target);
            self.quiet_windows = 0;
        } else if self.window_late + self.window_underflow == 0 {
            self.quiet_windows += 1;
            if self.quiet_windows >= 6 {
                self.target = (self.target - 1).max(self.min_target);
                self.quiet_windows = 0;
            }
        }
        self.window_late = 0;
        self.window_underflow = 0;
        self.stats.target = self.target;
    }

    pub fn stats(&self) -> JitterStats {
        self.stats
    }

    pub fn target(&self) -> u32 {
        self.target
    }

    pub fn is_idle(&self) -> bool {
        self.next.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkt(n: u8) -> Vec<u8> {
        vec![n; 4]
    }

    #[test]
    fn prefill_then_in_order() {
        let mut jb = JitterBuffer::new(2);
        jb.push(10, true, &pkt(10));
        assert_eq!(jb.pop(), Playout::Idle); // prefill: 1 < 2
        jb.push(11, false, &pkt(11));
        assert_eq!(jb.pop(), Playout::Packet(pkt(10)));
        assert_eq!(jb.pop(), Playout::Packet(pkt(11)));
    }

    #[test]
    fn reorder_is_fixed() {
        let mut jb = JitterBuffer::new(2);
        jb.push(1, true, &pkt(1));
        jb.push(3, false, &pkt(3));
        jb.push(2, false, &pkt(2));
        assert_eq!(jb.pop(), Playout::Packet(pkt(1)));
        assert_eq!(jb.pop(), Playout::Packet(pkt(2)));
        assert_eq!(jb.pop(), Playout::Packet(pkt(3)));
    }

    #[test]
    fn loss_offers_fec_source_then_continues() {
        let mut jb = JitterBuffer::new(2);
        jb.push(1, true, &pkt(1));
        jb.push(3, false, &pkt(3)); // 2 lost
        assert_eq!(jb.pop(), Playout::Packet(pkt(1)));
        assert_eq!(jb.pop(), Playout::Lost { next: Some(pkt(3)) });
        assert_eq!(jb.pop(), Playout::Packet(pkt(3)));
        assert_eq!(jb.stats().lost, 1);
        assert_eq!(jb.stats().fec_recovered, 1);
    }

    #[test]
    fn goes_idle_after_gap_and_restarts() {
        let mut jb = JitterBuffer::new(1);
        jb.push(5, true, &pkt(5));
        assert_eq!(jb.pop(), Playout::Packet(pkt(5)));
        assert_eq!(jb.pop(), Playout::Lost { next: None }); // 1st miss (PLC)
        assert_eq!(jb.pop(), Playout::Lost { next: None }); // 2.
        assert_eq!(jb.pop(), Playout::Idle); // 3rd → idle
        assert!(jb.is_idle());
        jb.push(40, false, &pkt(40)); // restarts even without talk_start
        assert_eq!(jb.pop(), Playout::Packet(pkt(40)));
    }

    #[test]
    fn late_packet_counted_and_dropped() {
        let mut jb = JitterBuffer::new(1);
        jb.push(1, true, &pkt(1));
        jb.push(2, false, &pkt(2));
        assert_eq!(jb.pop(), Playout::Packet(pkt(1)));
        assert_eq!(jb.pop(), Playout::Packet(pkt(2)));
        jb.push(1, false, &pkt(1)); // late
        assert_eq!(jb.stats().late, 1);
        assert_eq!(jb.pop(), Playout::Lost { next: None });
    }

    #[test]
    fn seq_wraps_around() {
        let mut jb = JitterBuffer::new(1);
        jb.push(65534, true, &pkt(1));
        jb.push(65535, false, &pkt(2));
        jb.push(0, false, &pkt(3));
        jb.push(1, false, &pkt(4));
        for n in 1..=4 {
            assert_eq!(jb.pop(), Playout::Packet(pkt(n)));
        }
    }

    #[test]
    fn vad_pause_is_not_underflow_but_stall_is() {
        // Silence: 2 misses, then a new talk spurt with TALK_START → underflow 0.
        let mut jb = JitterBuffer::new(1);
        jb.push(1, true, &pkt(1));
        jb.pop();
        jb.pop();
        jb.pop();
        jb.push(2, true, &pkt(2));
        assert_eq!(jb.stats().underflow, 0);
        // Network stall: 2 misses, then continues in sequence → underflow 2.
        jb.pop(); // 2
        assert_eq!(jb.pop(), Playout::Lost { next: None });
        assert_eq!(jb.pop(), Playout::Lost { next: None });
        jb.push(3, false, &pkt(3)); // late (3 was skipped) → evidence of delay
        assert_eq!(jb.stats().underflow, 2);
        assert_eq!(jb.stats().late, 1);
    }

    #[test]
    fn adapt_grows_on_trouble_and_shrinks_when_quiet() {
        let mut jb = JitterBuffer::new(2);
        jb.push(1, true, &pkt(1));
        jb.push(2, false, &pkt(2));
        jb.pop();
        jb.pop();
        for _ in 0..3 {
            jb.push(1, false, &pkt(1)); // late ×3
        }
        jb.adapt();
        assert_eq!(jb.target(), 3);
        for _ in 0..6 {
            jb.adapt();
        }
        assert_eq!(jb.target(), 2);
    }

    #[test]
    fn delay_estimate_sets_target_from_spread() {
        let mut jb = JitterBuffer::new(3);
        // Steady 30 ms delay → no spread → minimum target.
        for i in 0..200u32 {
            jb.push_timed(i as u16, i * 960, i == 0, &pkt(1), i as f64 * 20.0 + 30.0);
        }
        jb.adapt();
        assert_eq!(jb.target(), 2);
        // Every 10th packet 50 ms late (10% > 5% tail) → target covers 50 ms: 3 + 1 frames.
        let mut jb = JitterBuffer::new(2);
        for i in 0..200u32 {
            let extra = if i % 10 == 5 { 50.0 } else { 0.0 };
            jb.push_timed(
                i as u16,
                i * 960,
                i == 0,
                &pkt(1),
                i as f64 * 20.0 + 30.0 + extra,
            );
        }
        jb.adapt();
        assert_eq!(jb.target(), 4);
    }

    #[test]
    fn estimator_ignores_preroll_burst_and_clock_restart() {
        let mut est = DelayEstimator::new();
        // Pre-roll: 3 old frames arrive together with the current one at talk start.
        for (k, ts) in [0u32, 960, 1920, 2880].iter().enumerate() {
            est.observe(*ts, 80.0, k == 0);
        }
        for i in 4..120u32 {
            est.observe(i * 960, i as f64 * 20.0 + 20.0, false);
        }
        assert!(est.spread_ms().unwrap() < 1.0, "pre-roll counted as jitter");
        // Sender pipeline restarted: timestamps jump back to 0; the window starts over.
        for i in 0..120u32 {
            est.observe(i * 960, 100_000.0 + i as f64 * 20.0, false);
        }
        assert!(est.spread_ms().unwrap() < 1.0);
    }

    #[test]
    fn resync_after_stall_trims_stale_backlog() {
        let mut jb = JitterBuffer::new(2);
        jb.push(1, true, &pkt(1));
        jb.push(2, false, &pkt(2));
        assert_eq!(jb.pop(), Playout::Packet(pkt(1)));
        assert_eq!(jb.pop(), Playout::Packet(pkt(2)));
        // Network stall: three empty ticks → idle.
        jb.pop();
        jb.pop();
        assert_eq!(jb.pop(), Playout::Idle);
        // The stalled packets arrive in one clump (no TALK_START: the sender kept talking).
        for s in 3..=9u16 {
            jb.push(s, false, &pkt(s as u8));
        }
        // Playout resumes near the live edge (target 2) instead of 7 frames behind.
        assert_eq!(jb.pop(), Playout::Packet(pkt(8)));
        assert_eq!(jb.pop(), Playout::Packet(pkt(9)));
    }
}
