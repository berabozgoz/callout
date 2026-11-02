//! Engine: receives commands, manages the network and the audio pipeline, emits events.
//!
//! Threads:
//! - **engine** (this module's tokio runtime): command loop, QUIC control
//!   stream, datagram send/receive tasks;
//! - **audio** (`callout-audio-pipeline`): sets up the cpal streams; microphone → VAD/PTT
//!   → Opus → network; network → jitter → Opus → mixer → speaker;
//! - cpal callbacks: ring buffer only.
//!
//! The audio thread lives while connected; while disconnected it only runs when settings are
//! open (preview: for the level meter and the sound test). It is rebuilt when the device changes.

use crate::{Command, ConnectionState, ErrorKind, Event, MicTestState};
use bytes::Bytes;
use callout_audio::codec::{Decoder, Encoder, EncoderConfig, FRAME};
use callout_audio::denoise::Denoiser;
use callout_audio::framer::Framer;
use callout_audio::jitter::{JitterBuffer, Playout};
use callout_audio::mixer::Mixer;
use callout_audio::resample::Resampler;
use callout_audio::vad::Vad;
use callout_audio::{find_device, Direction};
use callout_net::cert::Invite;
use callout_net::client::Client;
use callout_net::identity::DeviceKey;
use callout_proto::{
    flags, AudioPacket, ClientMsg, Member, RoomInfo, ServerMsg, UserId, FRAME_SAMPLES,
    SAMPLE_RATE_HZ,
};
use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::HeapRb;
use std::collections::{HashMap, VecDeque};
use std::f32::consts::TAU;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

const JITTER_TARGET: u32 = 3;
/// Microphone test recording: 250 frames = 5 s.
const MIC_TEST_FRAMES: usize = 250;
/// Voice activity detection: how long the gate stays open after speech ends, and how much
/// audio from before the start is sent retroactively.
const VAD_HANGOVER_MS: u32 = 400;
const VAD_PREROLL_MS: u32 = 60;
const PREROLL_FRAMES: usize = (VAD_PREROLL_MS / 20) as usize;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Folder holding the device key and settings.
    pub data_dir: PathBuf,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub push_to_talk: bool,
    /// Apply noise suppression (RNNoise) to the microphone signal.
    pub noise_suppression: bool,
    /// Never set up the audio pipeline (for test environments without audio devices).
    pub no_audio: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("callout-data"),
            input_device: None,
            output_device: None,
            push_to_talk: false,
            noise_suppression: true,
            no_audio: false,
        }
    }
}

/// Engine handle. The engine stops when it is dropped.
pub struct Engine {
    cmd_tx: mpsc::UnboundedSender<Command>,
}

impl Engine {
    /// Starts the engine on its own thread; events are read from `rx`.
    pub fn start(cfg: EngineConfig) -> (Engine, mpsc::UnboundedReceiver<Event>) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (ev_tx, ev_rx) = mpsc::unbounded_channel();
        std::thread::Builder::new()
            .name("callout-engine".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("tokio");
                rt.block_on(run(cfg, cmd_rx, ev_tx));
            })
            .expect("engine thread");
        (Engine { cmd_tx }, ev_rx)
    }

    pub fn send(&self, cmd: Command) {
        let _ = self.cmd_tx.send(cmd);
    }

    /// Cloneable sender for sending commands from other threads.
    pub fn sender(&self) -> mpsc::UnboundedSender<Command> {
        self.cmd_tx.clone()
    }
}

// ---------------------------------------------------------------------------
// Control shared with the audio pipeline
// ---------------------------------------------------------------------------

struct AudioControl {
    stop: AtomicBool,
    muted: AtomicBool,
    deafened: AtomicBool,
    ptt_mode: AtomicBool,
    ptt_pressed: AtomicBool,
    noise_suppression: AtomicBool,
    /// Set by the cpal stream error callback (device unplugged/lost); the engine rebuilds the audio pipeline.
    stream_error: AtomicBool,
    /// Audio is sent only while in a room (outside a room the server drops it anyway; keep it off the network).
    in_room: AtomicBool,
    /// Sound test requests: the engine sets them, the audio thread reads and clears them.
    mic_test_start: AtomicBool,
    mic_test_stop: AtomicBool,
    test_sound: AtomicBool,
    gains: Mutex<HashMap<UserId, f32>>,
    // statistics
    sent: AtomicU32,
    received: AtomicU32,
    lost: AtomicU32,
    jitter_target: AtomicU32,
}

impl AudioControl {
    fn new(ptt: bool, noise_suppression: bool) -> Arc<Self> {
        Arc::new(Self {
            noise_suppression: AtomicBool::new(noise_suppression),
            stop: AtomicBool::new(false),
            muted: AtomicBool::new(false),
            deafened: AtomicBool::new(false),
            ptt_mode: AtomicBool::new(ptt),
            ptt_pressed: AtomicBool::new(false),
            in_room: AtomicBool::new(false),
            stream_error: AtomicBool::new(false),
            mic_test_start: AtomicBool::new(false),
            mic_test_stop: AtomicBool::new(false),
            test_sound: AtomicBool::new(false),
            gains: Mutex::new(HashMap::new()),
            sent: AtomicU32::new(0),
            received: AtomicU32::new(0),
            lost: AtomicU32::new(0),
            jitter_target: AtomicU32::new(JITTER_TARGET),
        })
    }
}

struct Receiver {
    decoder: Decoder,
    jitter: JitterBuffer,
    last_packet: Instant,
}

/// Microphone test: while recording, frames pass through the transmit gate and Opus (`None`
/// when the gate is closed = silence for the room), then are decoded and played back as is.
enum MicTest {
    Off,
    /// Recording does not start until VAD calibration is done (no threshold yet).
    Waiting,
    Recording {
        enc: Box<Encoder>,
        frames: Vec<Option<Vec<u8>>>,
    },
    Playing {
        dec: Decoder,
        frames: Vec<Option<Vec<u8>>>,
        pos: usize,
        pct: u8,
    },
}

impl MicTest {
    fn recording() -> Self {
        MicTest::Recording {
            enc: Box::new(Encoder::new(EncoderConfig::default()).expect("opus encoder")),
            frames: Vec::with_capacity(MIC_TEST_FRAMES),
        }
    }
}

/// Encodes a frame; `None` for DTX silence (a packet that would not be sent) or on error.
fn encode_opus(enc: &mut Encoder, frame: &[f32]) -> Option<Vec<u8>> {
    enc.encode(frame)
        .ok()
        .filter(|p| !Encoder::is_dtx_packet(p))
        .map(|p| p.to_vec())
}

/// Short two-note chime (48 kHz, length a multiple of the frame size).
fn test_sound() -> Vec<f32> {
    let sr = SAMPLE_RATE_HZ as f32;
    let len = ((0.8 * sr) as usize).div_ceil(FRAME) * FRAME;
    (0..len)
        .map(|i| {
            let t = i as f32 / sr;
            let mut s = 0.0;
            for (start, hz) in [(0.0f32, 880.0f32), (0.15, 1318.5)] {
                let dt = t - start;
                if dt >= 0.0 {
                    // 5 ms fade-in, exponential decay
                    s += (TAU * hz * dt).sin() * (-dt * 6.0).exp() * (dt * 200.0).min(1.0);
                }
            }
            0.25 * s
        })
        .collect()
}

/// Audio pipeline thread; runs until `ctl.stop` is true.
fn audio_thread(
    input_name: Option<String>,
    output_name: Option<String>,
    ctl: Arc<AudioControl>,
    tx_net: mpsc::UnboundedSender<Bytes>,
    rx_net: std::sync::mpsc::Receiver<Bytes>,
    ev: mpsc::UnboundedSender<Event>,
    ready: std::sync::mpsc::Sender<Result<(), String>>,
) {
    let setup = (|| -> Result<_, String> {
        let input = find_device(Direction::Input, input_name.as_deref())
            .map_err(|e| format!("input device: {e}"))?;
        let output = find_device(Direction::Output, output_name.as_deref())
            .map_err(|e| format!("output device: {e}"))?;
        let in_cfg = input.default_input_config().map_err(|e| e.to_string())?;
        let out_cfg = output.default_output_config().map_err(|e| e.to_string())?;
        Ok((input, output, in_cfg, out_cfg))
    })();
    let (input, output, in_cfg, out_cfg) = match setup {
        Ok(v) => v,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let (in_rate, out_rate) = (in_cfg.sample_rate().0, out_cfg.sample_rate().0);
    let (in_ch, out_ch) = (in_cfg.channels() as usize, out_cfg.channels() as usize);

    let (mut in_prod, mut in_cons) = HeapRb::<f32>::new(in_rate as usize / 5).split();
    let (mut out_prod, mut out_cons) = HeapRb::<f32>::new(out_rate as usize / 5).split();

    // Error callbacks only set the flag (logging the first error); the engine handles recovery.
    let (err_in, err_out) = (ctl.clone(), ctl.clone());
    let streams = (|| -> Result<_, String> {
        let in_stream = input
            .build_input_stream(
                &in_cfg.clone().into(),
                move |data: &[f32], _| {
                    for frame in data.chunks(in_ch) {
                        let mono = frame.iter().sum::<f32>() / in_ch as f32;
                        let _ = in_prod.try_push(mono);
                    }
                },
                move |e| {
                    if !err_in.stream_error.swap(true, Ordering::Relaxed) {
                        tracing::error!("input error: {e}");
                    }
                },
                None,
            )
            .map_err(|e| format!("input stream: {e}"))?;
        let out_stream = output
            .build_output_stream(
                &out_cfg.clone().into(),
                move |data: &mut [f32], _| {
                    for frame in data.chunks_mut(out_ch) {
                        let s = out_cons.try_pop().unwrap_or(0.0);
                        frame.fill(s);
                    }
                },
                move |e| {
                    if !err_out.stream_error.swap(true, Ordering::Relaxed) {
                        tracing::error!("output error: {e}");
                    }
                },
                None,
            )
            .map_err(|e| format!("output stream: {e}"))?;
        in_stream.play().map_err(|e| e.to_string())?;
        out_stream.play().map_err(|e| e.to_string())?;
        Ok((in_stream, out_stream))
    })();
    let _streams = match streams {
        Ok(s) => s,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let _ = ready.send(Ok(()));
    tracing::info!(in_rate, out_rate, in_ch, out_ch, "audio pipeline started");

    let mut up = Resampler::new(in_rate, SAMPLE_RATE_HZ).expect("resampler");
    let mut down = Resampler::new(SAMPLE_RATE_HZ, out_rate).expect("resampler");
    let mut framer = Framer::new();
    let mut vad = Vad::new(-45.0, VAD_HANGOVER_MS, VAD_PREROLL_MS);
    let mut denoiser = Denoiser::new();
    let mut ns_active = ctl.noise_suppression.load(Ordering::Relaxed);
    // Pre-roll buffer: the latest frames (including the current one). When speech starts, the earlier ones are sent too.
    let mut history: VecDeque<[f32; FRAME]> = VecDeque::with_capacity(PREROLL_FRAMES + 1);
    // Speech started but no packet has gone out yet (the first frames may be DTX): TALK_START is pending.
    let mut pending_start = false;
    let mut calib: Vec<f32> = Vec::with_capacity(75);
    let mut enc = Encoder::new(EncoderConfig::default()).expect("opus encoder");
    let mut receivers: HashMap<UserId, Receiver> = HashMap::new();
    let started = Instant::now();
    let mut mixer = Mixer::new();

    let mut chunk = Vec::with_capacity(1024);
    let mut at48 = Vec::with_capacity(4096);
    let mut frame = [0f32; FRAME];
    let mut pkt_buf = Vec::with_capacity(512);
    let mut seq: u16 = 0;
    let mut timestamp: u32 = 0;
    let mut mix = vec![0f32; FRAME];
    let mut decoded = vec![0f32; FRAME];
    let mut out = Vec::with_capacity(4096);
    let out_low_water = (out_rate as usize) * 40 / 1000;
    let mut last_adapt = Instant::now();
    let mut adapt_ticks = 0u32;
    // Losses of senders removed from the table after going silent (so the total counter never goes down).
    let mut lost_retired = 0u32;
    let mut last_level = Instant::now();
    let mut level_peak = 0f32;
    let mut was_transmitting = false;
    let mut mic_test = MicTest::Off;
    // For the level meter: whether the gate is open during the test recording (even though nothing is sent to the room).
    let mut test_gate = false;
    let mut tone: Vec<f32> = Vec::new();
    let mut tone_pos = 0usize;
    let emit_test = |state: MicTestState, transmitted_pct: Option<u8>| {
        let _ = ev.send(Event::MicTest {
            state,
            transmitted_pct,
        });
    };

    while !ctl.stop.load(Ordering::Relaxed) {
        // 0) Sound test requests
        if ctl.mic_test_stop.swap(false, Ordering::Relaxed) && !matches!(mic_test, MicTest::Off) {
            mic_test = MicTest::Off;
            test_gate = false;
            emit_test(MicTestState::Idle, None);
        }
        if ctl.mic_test_start.swap(false, Ordering::Relaxed) {
            if calib.len() < 75 {
                mic_test = MicTest::Waiting;
                emit_test(MicTestState::Calibrating, None);
            } else {
                mic_test = MicTest::recording();
                emit_test(MicTestState::Recording, None);
            }
        }
        if ctl.test_sound.swap(false, Ordering::Relaxed) {
            tone = test_sound();
            tone_pos = 0;
        }

        // 1) Transmit
        chunk.clear();
        while let Some(s) = in_cons.try_pop() {
            chunk.push(s);
            if chunk.len() == 1024 {
                break;
            }
        }
        if !chunk.is_empty() {
            at48.clear();
            if up.process(&chunk, &mut at48).is_err() {
                continue;
            }
            framer.push(&at48);
            while framer.next_frame(&mut frame) {
                let ns = ctl.noise_suppression.load(Ordering::Relaxed);
                if ns != ns_active {
                    // Denoised and raw audio have very different noise floors: re-measure the threshold.
                    ns_active = ns;
                    calib.clear();
                    denoiser = Denoiser::new();
                    tracing::info!(enabled = ns, "noise suppression");
                }
                let speech_prob = ns_active.then(|| denoiser.process(&mut frame));
                let rms = Vad::rms(&frame);
                level_peak = level_peak.max(rms);
                timestamp = timestamp.wrapping_add(FRAME_SAMPLES);

                if calib.len() < 75 {
                    calib.push(Vad::to_db(rms));
                    if calib.len() == 75 {
                        calib.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        let floor = calib[calib.len() / 2];
                        vad.set_adaptive(floor, 6.0);
                        tracing::info!(floor, threshold = vad.threshold_db(), "VAD calibrated");
                        if matches!(mic_test, MicTest::Waiting) {
                            mic_test = MicTest::recording();
                            emit_test(MicTestState::Recording, None);
                        }
                    }
                    continue;
                }

                let d = vad.process_with_prob(&frame, speech_prob);
                let ptt = ctl.ptt_mode.load(Ordering::Relaxed);
                if history.len() > PREROLL_FRAMES {
                    history.pop_front();
                }
                history.push_back(frame);
                // Number of frames to send retroactively if voice detection opened the gate.
                let preroll = if ptt {
                    0
                } else {
                    (d.preroll as usize).min(history.len() - 1)
                };
                let hist_end = history.len() - 1; // the last element is this frame

                if let MicTest::Recording { enc, frames } = &mut mic_test {
                    // In push-to-talk mode the test records without the key (the shortcut is not registered outside a room).
                    test_gate = ptt || d.send;
                    // Speech started: fill the last frames that were skipped while the gate was closed from the pre-roll buffer.
                    let n = preroll.min(frames.len());
                    if n > 0 {
                        let start = frames.len() - n;
                        for (slot, f) in frames[start..]
                            .iter_mut()
                            .zip(history.range(hist_end - n..hist_end))
                        {
                            *slot = encode_opus(enc, f);
                        }
                    }
                    let opus = if test_gate {
                        encode_opus(enc, &frame)
                    } else {
                        None
                    };
                    frames.push(opus);
                    if frames.len() >= MIC_TEST_FRAMES {
                        let sent = frames.iter().filter(|f| f.is_some()).count();
                        let pct = (sent * 100 / frames.len()) as u8;
                        mic_test = MicTest::Playing {
                            dec: Decoder::new().expect("decoder"),
                            frames: std::mem::take(frames),
                            pos: 0,
                            pct,
                        };
                        test_gate = false;
                        emit_test(MicTestState::Playing, Some(pct));
                    }
                    // Do not send to the room while recording; afterwards a new talk spurt starts with TALK_START.
                    was_transmitting = false;
                    continue;
                }

                let gate = if ptt {
                    ctl.ptt_pressed.load(Ordering::Relaxed)
                } else {
                    d.send
                };
                let send = gate
                    && !ctl.muted.load(Ordering::Relaxed)
                    && ctl.in_room.load(Ordering::Relaxed);
                let talk_start = send && !was_transmitting;
                was_transmitting = send;
                if !send {
                    continue;
                }
                // If transmission just started because voice detection triggered, the earlier frames go too
                // (not on unmute: there is no need to reach back into speech already under way).
                let preroll = if talk_start && d.talk_start {
                    preroll
                } else {
                    0
                };
                pending_start |= talk_start;
                for (i, f) in history.range(hist_end - preroll..=hist_end).enumerate() {
                    let Ok(opus) = enc.encode(f) else {
                        continue;
                    };
                    if Encoder::is_dtx_packet(opus) {
                        continue;
                    }
                    seq = seq.wrapping_add(1);
                    let p = AudioPacket {
                        sender_id: 0,
                        seq,
                        timestamp: timestamp.wrapping_sub((preroll - i) as u32 * FRAME_SAMPLES),
                        flags: if pending_start { flags::TALK_START } else { 0 },
                        opus,
                    };
                    if p.write_to(&mut pkt_buf).is_ok() {
                        pending_start = false;
                        ctl.sent.fetch_add(1, Ordering::Relaxed);
                        let _ = tx_net.send(Bytes::copy_from_slice(&pkt_buf));
                    }
                }
            }
        }

        // 2) Receive
        while let Ok(bytes) = rx_net.try_recv() {
            let Ok(p) = AudioPacket::parse(&bytes) else {
                continue;
            };
            ctl.received.fetch_add(1, Ordering::Relaxed);
            let r = receivers.entry(p.sender_id).or_insert_with(|| Receiver {
                decoder: Decoder::new().expect("decoder"),
                jitter: JitterBuffer::new(JITTER_TARGET),
                last_packet: Instant::now(),
            });
            // Arrival time feeds the per-sender delay estimate (target depth). It is taken here,
            // not at the socket: the loop's few-ms latency is small against 20 ms frames.
            let now = Instant::now();
            let arrival_ms = now.duration_since(started).as_secs_f64() * 1000.0;
            r.jitter.push_timed(
                p.seq,
                p.timestamp,
                p.flags & flags::TALK_START != 0,
                p.opus,
                arrival_ms,
            );
            r.last_packet = now;
        }

        // 3) Mix
        if out_prod.occupied_len() < out_low_water {
            {
                let gains = ctl.gains.lock().unwrap();
                for (id, g) in gains.iter() {
                    mixer.set_gain(*id, *g);
                }
            }
            mixer.set_master(if ctl.deafened.load(Ordering::Relaxed) {
                0.0
            } else {
                1.0
            });
            mixer.begin();
            for (id, r) in receivers.iter_mut() {
                let ok = match r.jitter.pop() {
                    Playout::Packet(pkt) => r.decoder.decode(Some(&pkt), &mut decoded).is_ok(),
                    Playout::Lost { next: Some(nxt) } => {
                        r.decoder.decode_fec(&nxt, &mut decoded).is_ok()
                    }
                    Playout::Lost { next: None } => r.decoder.decode(None, &mut decoded).is_ok(),
                    Playout::Idle => false,
                };
                if ok {
                    mixer.add(*id, &decoded);
                }
            }
            mixer.finish(&mut mix);
            // The sound test is added on top of the mix (audible even when deafened: it is a test).
            let mut test_done = None;
            if let MicTest::Playing {
                dec,
                frames,
                pos,
                pct,
            } = &mut mic_test
            {
                let ok = match &frames[*pos] {
                    Some(p) => dec.decode(Some(p), &mut decoded).is_ok(),
                    None => false,
                };
                if ok {
                    for (m, s) in mix.iter_mut().zip(&decoded) {
                        *m = (*m + s).clamp(-1.0, 1.0);
                    }
                }
                *pos += 1;
                if *pos >= frames.len() {
                    test_done = Some(*pct);
                }
            }
            if let Some(pct) = test_done {
                mic_test = MicTest::Off;
                emit_test(MicTestState::Idle, Some(pct));
            }
            if tone_pos < tone.len() {
                for (m, s) in mix.iter_mut().zip(&tone[tone_pos..tone_pos + FRAME]) {
                    *m = (*m + s).clamp(-1.0, 1.0);
                }
                tone_pos += FRAME;
            }
            out.clear();
            if down.process(&mix, &mut out).is_ok() {
                for &s in &out {
                    let _ = out_prod.try_push(s);
                }
            }
            if last_adapt.elapsed() >= Duration::from_secs(5) {
                last_adapt = Instant::now();
                adapt_ticks += 1;
                let mut max_target = 0;
                let mut lost = lost_retired;
                for r in receivers.values_mut() {
                    r.jitter.adapt();
                    let st = r.jitter.stats();
                    max_target = max_target.max(st.target);
                    lost += st.lost;
                }
                if !receivers.is_empty() {
                    ctl.jitter_target.store(max_target, Ordering::Relaxed);
                }
                ctl.lost.store(lost, Ordering::Relaxed);
                // Per-sender diagnostics (every 30 s, cumulative): whose packets arrive
                // irregularly (buffer target, late/lost/underflow counts). `lost - fec`
                // is the frames filled by PLC alone (consecutive losses).
                if adapt_ticks % 6 == 0 {
                    for (id, r) in &receivers {
                        let st = r.jitter.stats();
                        tracing::info!(
                            sender = id,
                            target_ms = st.target * 20,
                            late = st.late,
                            lost = st.lost,
                            fec = st.fec_recovered,
                            underflow = st.underflow,
                            "sender jitter"
                        );
                    }
                }
                receivers.retain(|_, r| {
                    let keep = r.last_packet.elapsed() < Duration::from_secs(60);
                    if !keep {
                        lost_retired += r.jitter.stats().lost;
                    }
                    keep
                });
            }
        } else {
            std::thread::sleep(Duration::from_millis(2));
        }

        // 4) Level event (~100 ms)
        if last_level.elapsed() >= Duration::from_millis(100) {
            last_level = Instant::now();
            // -60 dBFS → 0.0, 0 dBFS → 1.0
            let db = Vad::to_db(level_peak);
            let level = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
            level_peak = 0.0;
            let threshold = ((vad.threshold_db() + 60.0) / 60.0).clamp(0.0, 1.0);
            let _ = ev.send(Event::InputLevel {
                level,
                transmitting: was_transmitting || test_gate,
                threshold,
            });
        }
    }
    if !matches!(mic_test, MicTest::Off) {
        // The pipeline stopped during a test (preview closed, device changed): don't leave the UI waiting.
        emit_test(MicTestState::Idle, None);
    }
    tracing::info!("audio pipeline stopped");
}

// ---------------------------------------------------------------------------
// Engine loop
// ---------------------------------------------------------------------------

struct AudioHandle {
    ctl: Arc<AudioControl>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for AudioHandle {
    fn drop(&mut self) {
        self.ctl.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn start_audio(
    cfg: &EngineConfig,
    ctl: Arc<AudioControl>,
    tx_net: mpsc::UnboundedSender<Bytes>,
    rx_net: std::sync::mpsc::Receiver<Bytes>,
    ev: mpsc::UnboundedSender<Event>,
) -> Result<AudioHandle, String> {
    ctl.stop.store(false, Ordering::Relaxed);
    ctl.stream_error.store(false, Ordering::Relaxed);
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let (inp, outp, c) = (
        cfg.input_device.clone(),
        cfg.output_device.clone(),
        ctl.clone(),
    );
    let thread = std::thread::Builder::new()
        .name("callout-audio-pipeline".into())
        .spawn(move || audio_thread(inp, outp, c, tx_net, rx_net, ev, ready_tx))
        .map_err(|e| e.to_string())?;
    ready_rx
        .recv()
        .map_err(|_| "audio pipeline did not start".to_string())??;
    Ok(AudioHandle {
        ctl,
        thread: Some(thread),
    })
}

/// Local audio pipeline while disconnected (preview): the network channels are dangling, and
/// since `in_room` is off nothing is sent anyway. Emits an error event if it cannot be set up.
fn open_preview(
    cfg: &EngineConfig,
    ctl: &Arc<AudioControl>,
    ev: &mpsc::UnboundedSender<Event>,
) -> Option<AudioHandle> {
    if cfg.no_audio {
        return None;
    }
    let (tx_net, _) = mpsc::unbounded_channel::<Bytes>();
    let (_, rx_net) = std::sync::mpsc::channel::<Bytes>();
    match start_audio(cfg, ctl.clone(), tx_net, rx_net, ev.clone()) {
        Ok(a) => Some(a),
        Err(e) => {
            let _ = ev.send(Event::Error {
                kind: ErrorKind::from_audio_error(&e),
                message: format!("audio pipeline: {e}"),
            });
            None
        }
    }
}

async fn run(
    mut cfg: EngineConfig,
    mut cmd_rx: mpsc::UnboundedReceiver<Command>,
    ev: mpsc::UnboundedSender<Event>,
) {
    let emit = |e: Event| {
        let _ = ev.send(e);
    };
    let ctl = AudioControl::new(cfg.push_to_talk, cfg.noise_suppression);
    // The UI asks for a preview while on the settings screen; it persists after a disconnect too.
    let mut preview_wanted = false;

    loop {
        // ---------------- Disconnected ----------------
        emit(Event::ConnectionState {
            state: ConnectionState::Disconnected,
        });
        let mut preview = if preview_wanted {
            open_preview(&cfg, &ctl, &ev)
        } else {
            None
        };
        let (invite, nickname) = loop {
            match cmd_rx.recv().await {
                None => return,
                Some(Command::Connect { invite, nickname }) => break (invite, nickname),
                Some(Command::SetInputDevice { name }) => {
                    cfg.input_device = name;
                    if preview.take().is_some() {
                        preview = open_preview(&cfg, &ctl, &ev);
                    }
                }
                Some(Command::SetOutputDevice { name }) => {
                    cfg.output_device = name;
                    if preview.take().is_some() {
                        preview = open_preview(&cfg, &ctl, &ev);
                    }
                }
                Some(Command::SetAudioPreview { enabled }) => {
                    preview_wanted = enabled;
                    if !enabled {
                        preview = None;
                    } else if preview.is_none() {
                        preview = open_preview(&cfg, &ctl, &ev);
                    }
                }
                Some(Command::StartMicTest) => {
                    if preview.is_none() {
                        preview = open_preview(&cfg, &ctl, &ev);
                    }
                    if preview.is_some() {
                        ctl.mic_test_start.store(true, Ordering::Relaxed);
                    }
                }
                Some(Command::StopMicTest) => ctl.mic_test_stop.store(true, Ordering::Relaxed),
                Some(Command::PlayTestSound) => {
                    if preview.is_none() {
                        preview = open_preview(&cfg, &ctl, &ev);
                    }
                    if preview.is_some() {
                        ctl.test_sound.store(true, Ordering::Relaxed);
                    }
                }
                Some(Command::SetPushToTalkMode { enabled }) => {
                    ctl.ptt_mode.store(enabled, Ordering::Relaxed)
                }
                Some(Command::PushToTalk { pressed }) => {
                    ctl.ptt_pressed.store(pressed, Ordering::Relaxed)
                }
                Some(Command::SetMuted { muted }) => ctl.muted.store(muted, Ordering::Relaxed),
                Some(Command::SetNoiseSuppression { enabled }) => {
                    ctl.noise_suppression.store(enabled, Ordering::Relaxed)
                }
                Some(Command::SetDeafened { deafened }) => {
                    ctl.deafened.store(deafened, Ordering::Relaxed)
                }
                Some(Command::Disconnect) => {}
                Some(other) => emit(Event::Error {
                    kind: ErrorKind::RequestFailed,
                    message: format!("while disconnected: {other:?}"),
                }),
            }
        };

        // Close the preview before the real audio pipeline starts (same devices, same control).
        drop(preview);

        // ---------------- Connecting ----------------
        emit(Event::ConnectionState {
            state: ConnectionState::Connecting,
        });
        let invite = match Invite::parse(&invite) {
            Ok(i) => i,
            Err(e) => {
                emit(Event::Error {
                    kind: ErrorKind::InvalidInvite,
                    message: format!("invite link: {e}"),
                });
                continue;
            }
        };
        let key = match DeviceKey::load_or_generate(&cfg.data_dir.join("device.key")) {
            Ok(k) => k,
            Err(e) => {
                emit(Event::Error {
                    kind: ErrorKind::DeviceKey,
                    message: format!("device key: {e}"),
                });
                continue;
            }
        };
        let mut client = match Client::connect(&invite, &key, &nickname).await {
            Ok(c) => c,
            Err(e) => {
                emit(Event::Error {
                    kind: ErrorKind::ConnectFailed,
                    message: format!("could not connect: {e}"),
                });
                continue;
            }
        };
        let my_id = client.user_id();

        // Audio pipeline
        let (tx_net, mut rx_net_out) = mpsc::unbounded_channel::<Bytes>();
        let (tx_net_in, rx_net_in) = std::sync::mpsc::channel::<Bytes>();
        let mut audio = if cfg.no_audio {
            drop(rx_net_in);
            None
        } else {
            match start_audio(&cfg, ctl.clone(), tx_net.clone(), rx_net_in, ev.clone()) {
                Ok(a) => Some(a),
                Err(e) => {
                    emit(Event::Error {
                        kind: ErrorKind::from_audio_error(&e),
                        message: format!("audio pipeline: {e}"),
                    });
                    client.close();
                    continue;
                }
            }
        };

        // Network tasks
        let conn = client.connection();
        let send_task = tokio::spawn(async move {
            while let Some(pkt) = rx_net_out.recv().await {
                let _ = conn.send_datagram(pkt);
            }
        });
        let conn = client.connection();
        let (tx_net_in_arc, audio_restart_rx) = {
            // The receive channel changes when the audio pipeline restarts; the Mutex lets it be swapped.
            let shared = Arc::new(Mutex::new(tx_net_in));
            let (restart_tx, restart_rx) =
                mpsc::unbounded_channel::<std::sync::mpsc::Sender<Bytes>>();
            let s2 = shared.clone();
            tokio::spawn(async move {
                let mut rx = restart_rx;
                while let Some(new_tx) = rx.recv().await {
                    *s2.lock().unwrap() = new_tx;
                }
            });
            (shared, restart_tx)
        };
        let shared_in = tx_net_in_arc.clone();
        let recv_task = tokio::spawn(async move {
            while let Ok(bytes) = conn.read_datagram().await {
                let _ = shared_in.lock().unwrap().send(bytes);
            }
        });

        emit(Event::ConnectionState {
            state: ConnectionState::Connected,
        });
        emit(Event::Connected { user_id: my_id });

        let mut room: Option<RoomInfo> = None;
        let mut members: Vec<Member> = Vec::new();
        let mut stats = tokio::time::interval(Duration::from_secs(2));
        let mut stats_ticks = 0u32;
        let mut ping = tokio::time::interval(Duration::from_secs(3));
        // If the audio device disappears (headset unplugged, Bluetooth drop, default device change),
        // rebuild the pipeline automatically; on failure retry with exponential backoff.
        let mut audio_check = tokio::time::interval(Duration::from_secs(1));
        let mut recover_attempts = 0u32;
        let mut next_retry = Instant::now();
        let mut recovering = false;
        let mut last_restore = Instant::now();

        // ---------------- Connected ----------------
        let reason: String = loop {
            tokio::select! {
                cmd = cmd_rx.recv() => {
                    let Some(cmd) = cmd else { break "engine shut down".into() };
                    let r: Result<(), (ErrorKind, String)> = match cmd {
                        Command::Disconnect => break "user disconnected".into(),
                        // The UI may have lost its state (e.g. the webview was reloaded): don't error,
                        // re-emit the current state.
                        Command::Connect { .. } => {
                            emit(Event::ConnectionState { state: ConnectionState::Connected });
                            emit(Event::Connected { user_id: my_id });
                            emit(Event::RoomUpdated { room: room.clone(), members: members.clone() });
                            Ok(())
                        }
                        Command::CreateRoom { name } => client.create_room(&name).await.map_err(request_err),
                        Command::JoinRoom { code } => client.join_room(&code).await.map_err(request_err),
                        Command::LeaveRoom => {
                            // The server sends no confirmation to the leaver; clear local state right away.
                            let r = client.leave_room().await.map_err(request_err);
                            if room.is_some() {
                                room = None;
                                ctl.in_room.store(false, Ordering::Relaxed);
                                members.clear();
                                emit(Event::RoomUpdated { room: None, members: Vec::new() });
                            }
                            r
                        }
                        Command::SetMuted { muted } => {
                            ctl.muted.store(muted, Ordering::Relaxed);
                            client.control().send(&ClientMsg::SetMuted(muted)).await.map_err(request_err)
                        }
                        Command::SetDeafened { deafened } => {
                            ctl.deafened.store(deafened, Ordering::Relaxed);
                            client.control().send(&ClientMsg::SetDeafened(deafened)).await.map_err(request_err)
                        }
                        Command::SetPushToTalkMode { enabled } => { ctl.ptt_mode.store(enabled, Ordering::Relaxed); Ok(()) }
                        Command::PushToTalk { pressed } => { ctl.ptt_pressed.store(pressed, Ordering::Relaxed); Ok(()) }
                        Command::SetUserVolume { user_id, gain } => {
                            ctl.gains.lock().unwrap().insert(user_id, gain.clamp(0.0, 4.0));
                            Ok(())
                        }
                        Command::SetInputDevice { name } => {
                            cfg.input_device = name;
                            restart_audio(&cfg, &ctl, &tx_net, &audio_restart_rx, &ev, &mut audio).map_err(audio_err)
                        }
                        Command::SetOutputDevice { name } => {
                            cfg.output_device = name;
                            restart_audio(&cfg, &ctl, &tx_net, &audio_restart_rx, &ev, &mut audio).map_err(audio_err)
                        }
                        // While connected the pipeline is already open; just remember what to do after a disconnect.
                        Command::SetAudioPreview { enabled } => { preview_wanted = enabled; Ok(()) }
                        Command::StartMicTest if audio.is_none() => Err((ErrorKind::AudioFailed, "audio pipeline is not running".into())),
                        Command::StartMicTest => { ctl.mic_test_start.store(true, Ordering::Relaxed); Ok(()) }
                        Command::StopMicTest => { ctl.mic_test_stop.store(true, Ordering::Relaxed); Ok(()) }
                        Command::PlayTestSound => { ctl.test_sound.store(true, Ordering::Relaxed); Ok(()) }
                        Command::SetNoiseSuppression { enabled } => { ctl.noise_suppression.store(enabled, Ordering::Relaxed); Ok(()) }
                    };
                    if let Err((kind, message)) = r {
                        emit(Event::Error { kind, message });
                    }
                }
                msg = client.control().recv() => {
                    let msg = match msg {
                        Ok(m) => m,
                        // Report QUIC's actual reason instead of "connection lost" (timeout, reset…).
                        Err(e) => match client.connection().close_reason() {
                            Some(cause) => break format!("connection lost: {cause}"),
                            None => break format!("connection lost: {e}"),
                        },
                    };
                    match msg {
                        ServerMsg::RoomState { room: r, members: m } => {
                            room = Some(r);
                            ctl.in_room.store(true, Ordering::Relaxed);
                            members = m;
                            emit(Event::RoomUpdated { room: room.clone(), members: members.clone() });
                        }
                        ServerMsg::MemberJoined(m) => {
                            members.retain(|x| x.user_id != m.user_id);
                            members.push(m);
                            emit(Event::RoomUpdated { room: room.clone(), members: members.clone() });
                        }
                        ServerMsg::MemberLeft(id) => {
                            members.retain(|x| x.user_id != id);
                            emit(Event::RoomUpdated { room: room.clone(), members: members.clone() });
                        }
                        ServerMsg::MemberUpdated { user_id, muted, deafened } => {
                            if let Some(m) = members.iter_mut().find(|x| x.user_id == user_id) {
                                m.muted = muted;
                                m.deafened = deafened;
                            }
                            emit(Event::RoomUpdated { room: room.clone(), members: members.clone() });
                        }
                        ServerMsg::Speaking { user_id, speaking } => emit(Event::SpeakingChanged { user_id, speaking }),
                        ServerMsg::Error { code, message } => {
                            // Leaving a room is not confirmed by the server; errors other than NotInRoom go to the user.
                            emit(Event::Error { kind: ErrorKind::from_server(code), message: format!("{code:?}: {message}") });
                        }
                        ServerMsg::Pong(_) | ServerMsg::Welcome { .. } | ServerMsg::Authed => {}
                    }
                }
                _ = audio_check.tick() => {
                    if cfg.no_audio {
                        continue;
                    }
                    if !(ctl.stream_error.load(Ordering::Relaxed) || audio.is_none()) {
                        // The backoff resets after 30 s without problems (so a flapping device
                        // is not rebuilt immediately every time, flooding the log).
                        if last_restore.elapsed() > Duration::from_secs(30) {
                            recover_attempts = 0;
                        }
                        continue;
                    }
                    if !recovering {
                        recovering = true;
                        emit(Event::Error { kind: ErrorKind::AudioDeviceLost, message: "Audio device lost; reconnecting automatically…".into() });
                    }
                    if Instant::now() < next_retry {
                        continue;
                    }
                    recover_attempts += 1;
                    let wait = Duration::from_secs((1u64 << recover_attempts.min(4)).min(15));
                    let mut res = restart_audio(&cfg, &ctl, &tx_net, &audio_restart_rx, &ev, &mut audio);
                    let mut fallback = false;
                    if res.is_err() && (cfg.input_device.is_some() || cfg.output_device.is_some()) {
                        // The selected device is still missing: try the default devices (the selection stays in settings).
                        let default_cfg = EngineConfig { input_device: None, output_device: None, ..cfg.clone() };
                        res = restart_audio(&default_cfg, &ctl, &tx_net, &audio_restart_rx, &ev, &mut audio);
                        fallback = res.is_ok();
                    }
                    match res {
                        Ok(()) => {
                            tracing::info!(attempts = recover_attempts, fallback, "audio pipeline recovered");
                            recovering = false;
                            last_restore = Instant::now();
                            next_retry = last_restore + wait;
                            emit(Event::AudioRestored { fallback_to_default: fallback });
                        }
                        Err(e) => {
                            next_retry = Instant::now() + wait;
                            tracing::warn!(attempts = recover_attempts, wait_secs = wait.as_secs(), "could not set up audio pipeline: {e}");
                        }
                    }
                }
                _ = ping.tick() => {
                    if client.control().send(&ClientMsg::Ping(0)).await.is_err() {
                        break "ping failed".into();
                    }
                }
                _ = stats.tick() => {
                    let ev = Event::Stats {
                        rtt_ms: client.rtt().as_millis() as u32,
                        jitter_ms: ctl.jitter_target.load(Ordering::Relaxed) * 20,
                        lost: ctl.lost.load(Ordering::Relaxed),
                        sent: ctl.sent.swap(0, Ordering::Relaxed),
                        received: ctl.received.swap(0, Ordering::Relaxed),
                    };
                    // Session log: every 10 s (every 5th 2 s tick).
                    stats_ticks += 1;
                    if stats_ticks % 5 == 0 {
                        if let Event::Stats { rtt_ms, jitter_ms, lost, sent, received } = &ev {
                            tracing::info!(rtt_ms, jitter_ms, lost, sent, received, "stats (last 2 s: sent/received)");
                        }
                    }
                    emit(ev);
                }
            }
        };

        tracing::info!(reason, "closing connection");
        let _ = client.control().send(&ClientMsg::LeaveRoom).await;
        ctl.in_room.store(false, Ordering::Relaxed);
        client.close_graceful(Duration::from_millis(500)).await;
        send_task.abort();
        recv_task.abort();
        drop(audio);
        if room.is_some() {
            emit(Event::RoomUpdated {
                room: None,
                members: Vec::new(),
            });
        }
        if reason.starts_with("connection lost") || reason.starts_with("ping") {
            emit(Event::Error {
                kind: ErrorKind::ConnectionLost,
                message: reason.clone(),
            });
        }
        if reason == "engine shut down" {
            return;
        }
    }
}

/// A command's network/state failure, for [`Event::Error`].
fn request_err(e: impl std::fmt::Display) -> (ErrorKind, String) {
    (ErrorKind::RequestFailed, e.to_string())
}

/// An audio pipeline (re)start failure, for [`Event::Error`].
fn audio_err(e: String) -> (ErrorKind, String) {
    (
        ErrorKind::from_audio_error(&e),
        format!("audio pipeline: {e}"),
    )
}

fn restart_audio(
    cfg: &EngineConfig,
    ctl: &Arc<AudioControl>,
    tx_net: &mpsc::UnboundedSender<Bytes>,
    restart_tx: &mpsc::UnboundedSender<std::sync::mpsc::Sender<Bytes>>,
    ev: &mpsc::UnboundedSender<Event>,
    audio: &mut Option<AudioHandle>,
) -> Result<(), String> {
    if cfg.no_audio {
        return Ok(());
    }
    // Stop the old one (Drop), rebuild with a new channel.
    audio.take();
    let (tx_in, rx_in) = std::sync::mpsc::channel::<Bytes>();
    let _ = restart_tx.send(tx_in);
    *audio = Some(start_audio(
        cfg,
        ctl.clone(),
        tx_net.clone(),
        rx_in,
        ev.clone(),
    )?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sound_is_frame_aligned_and_safe() {
        let s = test_sound();
        assert!(!s.is_empty());
        assert_eq!(s.len() % FRAME, 0, "the mixer reads frame by frame");
        let peak = s.iter().fold(0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.1 && peak < 0.6, "peak {peak}");
        // Decays to the end: the last frame is nearly silent.
        let tail = s[s.len() - FRAME..]
            .iter()
            .fold(0f32, |m, x| m.max(x.abs()));
        assert!(tail < 0.05, "last frame peak {tail}");
    }
}
