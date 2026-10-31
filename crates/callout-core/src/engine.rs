//! Motor: komutları alır, ağ ve ses hattını yönetir, olay yayınlar.
//!
//! İş parçacıkları:
//! - **motor** (bu modülün tokio çalışma zamanı): komut döngüsü, QUIC kontrol
//!   akışı, datagram gönder/al görevleri;
//! - **ses** (`callout-audio-pipeline`): cpal akışlarını kurar; mikrofon → VAD/PTT
//!   → Opus → ağ; ağ → jitter → Opus → karıştırıcı → hoparlör;
//! - cpal geri çağrıları: yalnızca halka tampon.
//!
//! Ses iş parçacığı yalnızca bağlıyken yaşar; cihaz değişince yeniden kurulur.

use crate::{Command, ConnectionState, Event};
use bytes::Bytes;
use callout_audio::codec::{Decoder, Encoder, EncoderConfig, FRAME};
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
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

const JITTER_TARGET: u32 = 3;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Cihaz anahtarı ve ayarların yaşadığı klasör.
    pub data_dir: PathBuf,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub push_to_talk: bool,
    /// Ses hattını hiç kurma (ses cihazı olmayan test ortamları için).
    pub no_audio: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("callout-data"),
            input_device: None,
            output_device: None,
            push_to_talk: false,
            no_audio: false,
        }
    }
}

/// Motor tutamacı. Düşürülünce motor durur.
pub struct Engine {
    cmd_tx: mpsc::UnboundedSender<Command>,
}

impl Engine {
    /// Motoru kendi iş parçacığında başlatır; olaylar `rx`'ten okunur.
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
            .expect("motor iş parçacığı");
        (Engine { cmd_tx }, ev_rx)
    }

    pub fn send(&self, cmd: Command) {
        let _ = self.cmd_tx.send(cmd);
    }

    /// Başka iş parçacıklarından komut göndermek için klonlanabilir gönderici.
    pub fn sender(&self) -> mpsc::UnboundedSender<Command> {
        self.cmd_tx.clone()
    }
}

// ---------------------------------------------------------------------------
// Ses hattı ile paylaşılan kontrol
// ---------------------------------------------------------------------------

struct AudioControl {
    stop: AtomicBool,
    muted: AtomicBool,
    deafened: AtomicBool,
    ptt_mode: AtomicBool,
    ptt_pressed: AtomicBool,
    /// cpal akış hata geri çağrısı (cihaz çıkarıldı/kayboldu) bunu kurar; motor ses hattını yeniden kurar.
    stream_error: AtomicBool,
    /// Yalnızca odadayken ses gönderilir (oda dışında sunucu zaten atıyor; ağa çıkmasın).
    in_room: AtomicBool,
    gains: Mutex<HashMap<UserId, f32>>,
    // istatistik
    sent: AtomicU32,
    received: AtomicU32,
    lost: AtomicU32,
    jitter_target: AtomicU32,
}

impl AudioControl {
    fn new(ptt: bool) -> Arc<Self> {
        Arc::new(Self {
            stop: AtomicBool::new(false),
            muted: AtomicBool::new(false),
            deafened: AtomicBool::new(false),
            ptt_mode: AtomicBool::new(ptt),
            ptt_pressed: AtomicBool::new(false),
            in_room: AtomicBool::new(false),
            stream_error: AtomicBool::new(false),
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

/// Ses hattı iş parçacığı; `ctl.stop` true olana kadar döner.
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
            .map_err(|e| format!("giriş cihazı: {e}"))?;
        let output = find_device(Direction::Output, output_name.as_deref())
            .map_err(|e| format!("çıkış cihazı: {e}"))?;
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

    // Hata geri çağrıları yalnızca bayrağı kurar (ilk hatada günlüğe yazar); kurtarmayı motor yapar.
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
                        tracing::error!("giriş hatası: {e}");
                    }
                },
                None,
            )
            .map_err(|e| format!("giriş akışı: {e}"))?;
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
                        tracing::error!("çıkış hatası: {e}");
                    }
                },
                None,
            )
            .map_err(|e| format!("çıkış akışı: {e}"))?;
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
    tracing::info!(in_rate, out_rate, in_ch, out_ch, "ses hattı başladı");

    let mut up = Resampler::new(in_rate, SAMPLE_RATE_HZ).expect("resampler");
    let mut down = Resampler::new(SAMPLE_RATE_HZ, out_rate).expect("resampler");
    let mut framer = Framer::new();
    let mut vad = Vad::new(-45.0, 300);
    let mut calib: Vec<f32> = Vec::with_capacity(75);
    let mut enc = Encoder::new(EncoderConfig::default()).expect("opus encoder");
    let mut receivers: HashMap<UserId, Receiver> = HashMap::new();
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
    // Sessiz kalıp tablodan silinen gönderenlerin kaybı (toplam sayaç geri düşmesin).
    let mut lost_retired = 0u32;
    let mut last_level = Instant::now();
    let mut level_peak = 0f32;
    let mut was_transmitting = false;

    while !ctl.stop.load(Ordering::Relaxed) {
        // 1) Gönderim
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
                let rms = Vad::rms(&frame);
                level_peak = level_peak.max(rms);
                timestamp = timestamp.wrapping_add(FRAME_SAMPLES);

                if calib.len() < 75 {
                    calib.push(Vad::to_db(rms));
                    if calib.len() == 75 {
                        calib.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        let floor = calib[calib.len() / 2];
                        vad.set_adaptive(floor, 6.0);
                        tracing::info!(floor, threshold = vad.threshold_db(), "VAD kalibre edildi");
                    }
                    continue;
                }

                let d = vad.process(&frame);
                let ptt = ctl.ptt_mode.load(Ordering::Relaxed);
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
                let Ok(opus) = enc.encode(&frame) else {
                    continue;
                };
                if Encoder::is_dtx_packet(opus) {
                    continue;
                }
                seq = seq.wrapping_add(1);
                let p = AudioPacket {
                    sender_id: 0,
                    seq,
                    timestamp,
                    flags: if talk_start { flags::TALK_START } else { 0 },
                    opus,
                };
                if p.write_to(&mut pkt_buf).is_ok() {
                    ctl.sent.fetch_add(1, Ordering::Relaxed);
                    let _ = tx_net.send(Bytes::copy_from_slice(&pkt_buf));
                }
            }
        }

        // 2) Alım
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
            r.jitter
                .push(p.seq, p.flags & flags::TALK_START != 0, p.opus);
            r.last_packet = Instant::now();
        }

        // 3) Karıştırma
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
                // Gönderen başına teşhis (30 sn'de bir, birikimli): hangi gönderenin paketleri
                // düzensiz geliyor (tampon hedefi, geç/eksik/boşalma sayıları).
                if adapt_ticks % 6 == 0 {
                    for (id, r) in &receivers {
                        let st = r.jitter.stats();
                        tracing::info!(
                            sender = id,
                            target_ms = st.target * 20,
                            late = st.late,
                            lost = st.lost,
                            underflow = st.underflow,
                            "gönderen jitter"
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

        // 4) Seviye olayı (~100 ms)
        if last_level.elapsed() >= Duration::from_millis(100) {
            last_level = Instant::now();
            // -60 dBFS → 0.0, 0 dBFS → 1.0
            let db = Vad::to_db(level_peak);
            let level = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
            level_peak = 0.0;
            let _ = ev.send(Event::InputLevel {
                level,
                transmitting: was_transmitting,
            });
        }
    }
    tracing::info!("ses hattı durdu");
}

// ---------------------------------------------------------------------------
// Motor döngüsü
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
        .map_err(|_| "ses hattı başlamadı".to_string())??;
    Ok(AudioHandle {
        ctl,
        thread: Some(thread),
    })
}

async fn run(
    mut cfg: EngineConfig,
    mut cmd_rx: mpsc::UnboundedReceiver<Command>,
    ev: mpsc::UnboundedSender<Event>,
) {
    let emit = |e: Event| {
        let _ = ev.send(e);
    };
    let ctl = AudioControl::new(cfg.push_to_talk);

    loop {
        // ---------------- Bağlı değil ----------------
        emit(Event::ConnectionState {
            state: ConnectionState::Disconnected,
        });
        let (invite, nickname) = loop {
            match cmd_rx.recv().await {
                None => return,
                Some(Command::Connect { invite, nickname }) => break (invite, nickname),
                Some(Command::SetInputDevice { name }) => cfg.input_device = name,
                Some(Command::SetOutputDevice { name }) => cfg.output_device = name,
                Some(Command::SetPushToTalkMode { enabled }) => {
                    ctl.ptt_mode.store(enabled, Ordering::Relaxed)
                }
                Some(Command::PushToTalk { pressed }) => {
                    ctl.ptt_pressed.store(pressed, Ordering::Relaxed)
                }
                Some(Command::SetMuted { muted }) => ctl.muted.store(muted, Ordering::Relaxed),
                Some(Command::SetDeafened { deafened }) => {
                    ctl.deafened.store(deafened, Ordering::Relaxed)
                }
                Some(Command::Disconnect) => {}
                Some(other) => emit(Event::Error {
                    message: format!("bağlı değilken: {other:?}"),
                }),
            }
        };

        // ---------------- Bağlanıyor ----------------
        emit(Event::ConnectionState {
            state: ConnectionState::Connecting,
        });
        let invite = match Invite::parse(&invite) {
            Ok(i) => i,
            Err(e) => {
                emit(Event::Error {
                    message: format!("davet linki: {e}"),
                });
                continue;
            }
        };
        let key = match DeviceKey::load_or_generate(&cfg.data_dir.join("device.key")) {
            Ok(k) => k,
            Err(e) => {
                emit(Event::Error {
                    message: format!("cihaz anahtarı: {e}"),
                });
                continue;
            }
        };
        let mut client = match Client::connect(&invite, &key, &nickname).await {
            Ok(c) => c,
            Err(e) => {
                emit(Event::Error {
                    message: format!("bağlanılamadı: {e}"),
                });
                continue;
            }
        };
        let my_id = client.user_id();

        // Ses hattı
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
                        message: format!("ses hattı: {e}"),
                    });
                    client.close();
                    continue;
                }
            }
        };

        // Ağ görevleri
        let conn = client.connection();
        let send_task = tokio::spawn(async move {
            while let Some(pkt) = rx_net_out.recv().await {
                let _ = conn.send_datagram(pkt);
            }
        });
        let conn = client.connection();
        let (tx_net_in_arc, audio_restart_rx) = {
            // Ses hattı yeniden başlatıldığında alıcı kanalı değişir; Mutex ile değiştirilebilir.
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
        // Ses cihazı kaybolursa (çıkarılan kulaklık, Bluetooth düşmesi, varsayılan cihaz değişimi)
        // hattı kendiliğinden yeniden kur; başarısızsa üstel geri çekilmeyle dene.
        let mut audio_check = tokio::time::interval(Duration::from_secs(1));
        let mut recover_attempts = 0u32;
        let mut next_retry = Instant::now();
        let mut recovering = false;
        let mut last_restore = Instant::now();

        // ---------------- Bağlı ----------------
        let reason: String = loop {
            tokio::select! {
                cmd = cmd_rx.recv() => {
                    let Some(cmd) = cmd else { break "motor kapatıldı".into() };
                    let r: Result<(), String> = match cmd {
                        Command::Disconnect => break "kullanıcı ayrıldı".into(),
                        // Arayüz durumunu kaybetmiş olabilir (ör. webview yenilendi): hata verme,
                        // mevcut durumu yeniden yayınla.
                        Command::Connect { .. } => {
                            emit(Event::ConnectionState { state: ConnectionState::Connected });
                            emit(Event::Connected { user_id: my_id });
                            emit(Event::RoomUpdated { room: room.clone(), members: members.clone() });
                            Ok(())
                        }
                        Command::CreateRoom { name } => client.create_room(&name).await.map_err(|e| e.to_string()),
                        Command::JoinRoom { code } => client.join_room(&code).await.map_err(|e| e.to_string()),
                        Command::LeaveRoom => {
                            // Sunucu ayrılana onay yollamaz; yerel durumu hemen temizle.
                            let r = client.leave_room().await.map_err(|e| e.to_string());
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
                            client.control().send(&ClientMsg::SetMuted(muted)).await.map_err(|e| e.to_string())
                        }
                        Command::SetDeafened { deafened } => {
                            ctl.deafened.store(deafened, Ordering::Relaxed);
                            client.control().send(&ClientMsg::SetDeafened(deafened)).await.map_err(|e| e.to_string())
                        }
                        Command::SetPushToTalkMode { enabled } => { ctl.ptt_mode.store(enabled, Ordering::Relaxed); Ok(()) }
                        Command::PushToTalk { pressed } => { ctl.ptt_pressed.store(pressed, Ordering::Relaxed); Ok(()) }
                        Command::SetUserVolume { user_id, gain } => {
                            ctl.gains.lock().unwrap().insert(user_id, gain.clamp(0.0, 4.0));
                            Ok(())
                        }
                        Command::SetInputDevice { name } => {
                            cfg.input_device = name;
                            restart_audio(&cfg, &ctl, &tx_net, &audio_restart_rx, &ev, &mut audio)
                        }
                        Command::SetOutputDevice { name } => {
                            cfg.output_device = name;
                            restart_audio(&cfg, &ctl, &tx_net, &audio_restart_rx, &ev, &mut audio)
                        }
                    };
                    if let Err(message) = r {
                        emit(Event::Error { message });
                    }
                }
                msg = client.control().recv() => {
                    let msg = match msg {
                        Ok(m) => m,
                        // "connection lost" yerine QUIC'in asıl nedenini yaz (zaman aşımı, sıfırlama…).
                        Err(e) => match client.connection().close_reason() {
                            Some(cause) => break format!("bağlantı koptu: {cause}"),
                            None => break format!("bağlantı koptu: {e}"),
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
                            // Odadan çıkma sunucuda onaylanmaz; NotInRoom dışındaki hatalar kullanıcıya gider.
                            emit(Event::Error { message: format!("{code:?}: {message}") });
                        }
                        ServerMsg::Pong(_) | ServerMsg::Welcome { .. } | ServerMsg::Authed => {}
                    }
                }
                _ = audio_check.tick() => {
                    if cfg.no_audio {
                        continue;
                    }
                    if !(ctl.stream_error.load(Ordering::Relaxed) || audio.is_none()) {
                        // 30 sn sorunsuz çalışınca geri çekilme sıfırlanır (cihaz gidip gelirse
                        // her seferinde hemen yeniden kurup günlüğü doldurmasın).
                        if last_restore.elapsed() > Duration::from_secs(30) {
                            recover_attempts = 0;
                        }
                        continue;
                    }
                    if !recovering {
                        recovering = true;
                        emit(Event::Error { message: "Ses cihazı kayboldu; otomatik yeniden bağlanılıyor…".into() });
                    }
                    if Instant::now() < next_retry {
                        continue;
                    }
                    recover_attempts += 1;
                    let wait = Duration::from_secs((1u64 << recover_attempts.min(4)).min(15));
                    let mut res = restart_audio(&cfg, &ctl, &tx_net, &audio_restart_rx, &ev, &mut audio);
                    let mut fallback = false;
                    if res.is_err() && (cfg.input_device.is_some() || cfg.output_device.is_some()) {
                        // Seçili cihaz hâlâ yok: varsayılan cihazlarla dene (seçim ayarda kalır).
                        let default_cfg = EngineConfig { input_device: None, output_device: None, ..cfg.clone() };
                        res = restart_audio(&default_cfg, &ctl, &tx_net, &audio_restart_rx, &ev, &mut audio);
                        fallback = res.is_ok();
                    }
                    match res {
                        Ok(()) => {
                            tracing::info!(attempts = recover_attempts, fallback, "ses hattı kurtarıldı");
                            recovering = false;
                            last_restore = Instant::now();
                            next_retry = last_restore + wait;
                            emit(Event::AudioRestored { fallback_to_default: fallback });
                        }
                        Err(e) => {
                            next_retry = Instant::now() + wait;
                            tracing::warn!(attempts = recover_attempts, wait_secs = wait.as_secs(), "ses hattı kurulamadı: {e}");
                        }
                    }
                }
                _ = ping.tick() => {
                    if client.control().send(&ClientMsg::Ping(0)).await.is_err() {
                        break "ping gönderilemedi".into();
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
                    // Oturum günlüğü: 10 sn'de bir (2 sn'lik tikin her 5'i).
                    stats_ticks += 1;
                    if stats_ticks % 5 == 0 {
                        if let Event::Stats { rtt_ms, jitter_ms, lost, sent, received } = &ev {
                            tracing::info!(rtt_ms, jitter_ms, lost, sent, received, "istatistik (son 2 sn: gönderilen/alınan)");
                        }
                    }
                    emit(ev);
                }
            }
        };

        tracing::info!(reason, "bağlantı kapanıyor");
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
        if reason.starts_with("bağlantı koptu") || reason.starts_with("ping") {
            emit(Event::Error {
                message: reason.clone(),
            });
        }
        if reason == "motor kapatıldı" {
            return;
        }
    }
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
    // Eskiyi durdur (Drop), yeni kanalla yeniden kur.
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
