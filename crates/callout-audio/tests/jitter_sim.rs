//! Network delay-variation simulation for the jitter buffer.
//!
//! A sender emits a 20 ms frame per tick during talk spurts (VAD pauses in between);
//! each packet gets a random network delay; the receiver pops one frame per 20 ms tick
//! and adapts every 5 s, like the engine. Reports mouth-to-ear buffering latency, late
//! packets and the final target. See the numbers with:
//! `cargo test -p callout-audio --test jitter_sim -- --nocapture`

use callout_audio::jitter::{JitterBuffer, Playout};

const FRAME_MS: u64 = 20;

/// Deterministic LCG in [0, 1).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[derive(Clone, Copy)]
enum Net {
    /// Wired: 0–4 ms variation.
    Wired,
    /// Home Wi-Fi with gaming traffic: mostly <10 ms, sometimes 10–50 ms, ~3% spikes up to 120 ms.
    WiFi,
    /// Wi-Fi as seen at cocart (1 Nov): like `WiFi`, plus a 100–250 ms stall every ~8 s where
    /// packets queue up and then arrive together.
    WiFiStalls,
}

fn extra_delay(net: Net, rng: &mut Rng) -> f64 {
    let r = rng.next();
    match net {
        Net::Wired => rng.next() * 4.0,
        Net::WiFi | Net::WiFiStalls if r < 0.85 => rng.next() * 10.0,
        Net::WiFi | Net::WiFiStalls if r < 0.97 => 10.0 + rng.next() * 40.0,
        Net::WiFi | Net::WiFiStalls => 50.0 + rng.next() * 70.0,
    }
}

pub struct Report {
    pub avg_buffer_ms: f64,
    pub late_pct: f64,
    /// Concealed ticks, including the two PLC ticks at the end of every talk spurt (~1.3% here).
    pub plc_pct: f64,
    pub final_target_ms: u32,
}

/// `seconds` of conversation: 3 s talk spurts with 1 s pauses. `timed` feeds arrival times to the
/// buffer (delay estimate); without it the late/underflow heuristic adapts the target.
fn run(net: Net, seconds: u64, timed: bool) -> Report {
    let mut rng = Rng(42);
    let mut jb = JitterBuffer::new(3);
    let base = 30.0; // fixed one-way network delay, not counted as buffering
                     // (arrival time ms, seq, timestamp, talk_start, send time ms)
    let mut in_flight: Vec<(f64, u16, u32, bool, u64)> = Vec::new();
    let mut seq: u16 = 0;
    let mut sent = 0u64;
    let mut played = 0u64;
    let mut plc = 0u64;
    let mut buffer_sum = 0f64;
    let total_ms = seconds * 1000;
    let mut frame_idx: u64 = 0;
    // Current stall: packets sent before `stall_end` are held until then.
    let mut stall_end = 0f64;
    let mut next_stall = 8000.0 * rng.next();
    // Receiver ticks are offset from the sender's by 7 ms (unsynchronized clocks).
    for t in 0..total_ms {
        // sender
        if t % FRAME_MS == 0 {
            let in_spurt = (t % 4000) < 3000;
            let ts = (frame_idx * 960) as u32;
            frame_idx += 1;
            if in_spurt {
                let talk_start = t % 4000 == 0;
                seq = seq.wrapping_add(1);
                let mut arrive = t as f64 + base + extra_delay(net, &mut rng);
                if matches!(net, Net::WiFiStalls) {
                    if t as f64 >= next_stall {
                        stall_end = t as f64 + 100.0 + 150.0 * rng.next();
                        next_stall = t as f64 + 4000.0 + 8000.0 * rng.next();
                    }
                    if (t as f64) < stall_end {
                        arrive = arrive.max(stall_end + base);
                    }
                }
                in_flight.push((arrive, seq, ts, talk_start, t));
                sent += 1;
            }
        }
        // network delivery
        let now = t as f64;
        in_flight.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        while let Some(&(arrive, s, ts, start, send_t)) = in_flight.first() {
            if arrive > now {
                break;
            }
            in_flight.remove(0);
            let payload = send_t.to_le_bytes();
            if timed {
                jb.push_timed(s, ts, start, &payload, now);
            } else {
                jb.push(s, start, &payload);
            }
        }
        // receiver playout tick
        if t % FRAME_MS == 7 {
            match jb.pop() {
                Playout::Packet(p) => {
                    let send_t = u64::from_le_bytes(p[..8].try_into().unwrap());
                    played += 1;
                    // buffering = time from send to playout, minus the fixed network delay
                    buffer_sum += now - send_t as f64 - base;
                }
                Playout::Lost { .. } => plc += 1,
                Playout::Idle => {}
            }
        }
        if t % 5000 == 4999 {
            jb.adapt();
        }
    }
    let st = jb.stats();
    Report {
        avg_buffer_ms: buffer_sum / played.max(1) as f64,
        late_pct: 100.0 * st.late as f64 / sent as f64,
        plc_pct: 100.0 * plc as f64 / sent as f64,
        final_target_ms: st.target * 20,
    }
}

#[test]
fn delay_estimate_beats_counter_heuristic() {
    for (name, net) in [
        ("wired", Net::Wired),
        ("wifi", Net::WiFi),
        ("wifi+stalls", Net::WiFiStalls),
    ] {
        let old = run(net, 600, false);
        let new = run(net, 600, true);
        for (alg, r) in [("heuristic", &old), ("estimate ", &new)] {
            println!(
                "{name:12} {alg}: avg buffering {:4.0} ms, late {:.2}%, PLC {:.2}%, final target {} ms",
                r.avg_buffer_ms, r.late_pct, r.plc_pct, r.final_target_ms
            );
        }
        match net {
            Net::Wired => assert!(new.avg_buffer_ms < 50.0 && new.late_pct < 0.5),
            _ => {
                assert!(
                    new.avg_buffer_ms < old.avg_buffer_ms - 15.0,
                    "{name}: {:.0} ms vs {:.0} ms",
                    new.avg_buffer_ms,
                    old.avg_buffer_ms
                );
                assert!(new.late_pct < 2.5, "{name}: late {:.2}%", new.late_pct);
            }
        }
    }
}
