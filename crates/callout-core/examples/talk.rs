//! Headless konuşma istemcisi: Faz 1 adım 3'ün ölçüm aracı.
//!
//! Çalıştır:
//!   cargo run -p callout-core --example talk -- "callout://IP:4433#PARMAK_IZI" TakmaAd [--threshold-db -45]
//!
//! Hat: mikrofon → 48 kHz → VAD → Opus → datagram ─┐
//!      kulaklık ← karıştırıcı ← çözücü(gönderen) ← datagram ┘
//!
//! Alım tarafında küçük bir çalma kuyruğu vardır (gönderen başına 2–6 çerçeve);
//! gerçek jitter tamponu adım 4'te gelir.

use anyhow::{Context, Result};
use bytes::Bytes;
use callout_audio::codec::{Decoder, Encoder, EncoderConfig, FRAME};
use callout_audio::framer::Framer;
use callout_audio::resample::Resampler;
use callout_audio::vad::Vad;
use callout_audio::{find_device, Direction};
use callout_net::cert::Invite;
use callout_net::client::Client;
use callout_net::identity::DeviceKey;
use callout_proto::{
    flags, AudioPacket, ClientMsg, ServerMsg, UserId, FRAME_SAMPLES, SAMPLE_RATE_HZ,
};
use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::HeapRb;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

const MIN_QUEUE: usize = 2; // çalmaya başlamak için gereken çerçeve (40 ms)
const MAX_QUEUE: usize = 6; // bundan fazlası birikirse en eskisi düşer (120 ms)

struct Args {
    invite: Invite,
    nickname: String,
    /// `None` = otomatik kalibrasyon (gürültü tabanı + 6 dB, uyarlanabilir).
    threshold_db: Option<f32>,
}

fn parse_args() -> Result<Args> {
    let mut it = std::env::args().skip(1);
    let invite = Invite::parse(&it.next().context("1. argüman: callout:// davet linki")?)?;
    let nickname = it.next().context("2. argüman: takma ad")?;
    let mut threshold_db = None;
    while let Some(a) = it.next() {
        if a == "--threshold-db" {
            threshold_db = Some(it.next().context("--threshold-db değer")?.parse()?);
        }
    }
    Ok(Args {
        invite,
        nickname,
        threshold_db,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();
    let args = parse_args()?;

    let key = DeviceKey::load_or_generate(&PathBuf::from("callout-data/device.key"))?;
    let mut client = Client::connect(&args.invite, &key, &args.nickname)
        .await
        .context("sunucuya bağlanma")?;
    let my_id = client.user_id();

    // --- Ses cihazları ---
    let input = find_device(Direction::Input, None).context("giriş cihazı")?;
    let output = find_device(Direction::Output, None).context("çıkış cihazı")?;
    let in_cfg = input.default_input_config()?;
    let out_cfg = output.default_output_config()?;
    let (in_rate, out_rate) = (in_cfg.sample_rate().0, out_cfg.sample_rate().0);
    let (in_ch, out_ch) = (in_cfg.channels() as usize, out_cfg.channels() as usize);
    tracing::info!(
        in_rate,
        out_rate,
        in_ch,
        out_ch,
        threshold_db = ?args.threshold_db,
        "ses yapılandırması"
    );

    let (mut in_prod, mut in_cons) = HeapRb::<f32>::new(in_rate as usize / 5).split();
    let (mut out_prod, mut out_cons) = HeapRb::<f32>::new(out_rate as usize / 5).split();

    let underruns = Arc::new(AtomicU64::new(0));
    let un = underruns.clone();

    let in_stream = input.build_input_stream(
        &in_cfg.clone().into(),
        move |data: &[f32], _| {
            for frame in data.chunks(in_ch) {
                let mono = frame.iter().sum::<f32>() / in_ch as f32;
                let _ = in_prod.try_push(mono);
            }
        },
        |e| tracing::error!("giriş hatası: {e}"),
        None,
    )?;
    let out_stream = output.build_output_stream(
        &out_cfg.clone().into(),
        move |data: &mut [f32], _| {
            for frame in data.chunks_mut(out_ch) {
                let s = out_cons.try_pop().unwrap_or_else(|| {
                    un.fetch_add(1, Ordering::Relaxed);
                    0.0
                });
                frame.fill(s);
            }
        },
        |e| tracing::error!("çıkış hatası: {e}"),
        None,
    )?;

    // --- Kanallar: ses iş parçacığı ↔ tokio ---
    let (tx_audio_out, mut rx_audio_out) = mpsc::unbounded_channel::<Bytes>(); // ses → ağ
    let (tx_audio_in, rx_audio_in) = std::sync::mpsc::channel::<Bytes>(); // ağ → ses

    let sent_pkts = Arc::new(AtomicU64::new(0));
    let recv_pkts = Arc::new(AtomicU64::new(0));
    let (sp, rp) = (sent_pkts.clone(), recv_pkts.clone());
    let threshold_db = args.threshold_db;
    // İstatistik için: son 5 sn'de en yüksek mikrofon seviyesi (dBFS×100, i32), VAD açık çerçeve sayısı, eşik.
    let mic_peak_db = Arc::new(std::sync::atomic::AtomicI32::new(-10000));
    let vad_open = Arc::new(AtomicU64::new(0));
    let vad_threshold = Arc::new(std::sync::atomic::AtomicI32::new(0));
    let (mp, vo, vt) = (mic_peak_db.clone(), vad_open.clone(), vad_threshold.clone());

    // --- Ses iş parçacığı ---
    std::thread::Builder::new()
        .name("callout-audio".into())
        .spawn(move || -> Result<()> {
            let mut up = Resampler::new(in_rate, SAMPLE_RATE_HZ)?;
            let mut down = Resampler::new(SAMPLE_RATE_HZ, out_rate)?;
            let mut framer = Framer::new();
            let mut vad = Vad::new(threshold_db.unwrap_or(-45.0), 300);
            // Otomatik kalibrasyon: ilk 75 çerçevenin (1,5 sn) RMS'lerinden gürültü tabanı.
            let mut calib: Vec<f32> = if threshold_db.is_none() {
                Vec::with_capacity(75)
            } else {
                Vec::new()
            };
            let calibrating = threshold_db.is_none();
            vt.store((vad.threshold_db() * 100.0) as i32, Ordering::Relaxed);
            let mut enc = Encoder::new(EncoderConfig::default())?;
            let mut decoders: HashMap<UserId, (Decoder, VecDeque<Vec<f32>>, Instant)> =
                HashMap::new();

            let mut chunk = Vec::with_capacity(1024);
            let mut at48 = Vec::with_capacity(4096);
            let mut frame = [0f32; FRAME];
            let mut pkt_buf = Vec::with_capacity(512);
            let mut seq: u16 = 0;
            let mut timestamp: u32 = 0;
            let mut mix = vec![0f32; FRAME];
            let mut out = Vec::with_capacity(4096);
            let out_low_water = (out_rate as usize) * 40 / 1000; // 40 ms

            loop {
                // 1) Gönderim: mikrofon → VAD → Opus → ağ
                chunk.clear();
                while let Some(s) = in_cons.try_pop() {
                    chunk.push(s);
                    if chunk.len() == 1024 {
                        break;
                    }
                }
                if !chunk.is_empty() {
                    at48.clear();
                    up.process(&chunk, &mut at48)?;
                    framer.push(&at48);
                    while framer.next_frame(&mut frame) {
                        let rms_db = Vad::to_db(Vad::rms(&frame));
                        mp.fetch_max((rms_db * 100.0) as i32, Ordering::Relaxed);
                        if calibrating && calib.len() < 75 {
                            calib.push(rms_db);
                            if calib.len() == 75 {
                                calib.sort_by(|a, b| a.partial_cmp(b).unwrap());
                                let floor = calib[calib.len() / 2];
                                vad.set_adaptive(floor, 6.0);
                                let thr = vad.threshold_db();
                                vt.store((thr * 100.0) as i32, Ordering::Relaxed);
                                tracing::info!(
                                    noise_floor_db = format!("{floor:.1}"),
                                    threshold_db = format!("{thr:.1}"),
                                    "VAD kalibre edildi"
                                );
                            }
                            timestamp = timestamp.wrapping_add(FRAME_SAMPLES);
                            continue; // kalibrasyon sırasında gönderme
                        }
                        let d = vad.process(&frame);
                        if d.send {
                            vo.fetch_add(1, Ordering::Relaxed);
                        }
                        if calibrating {
                            vt.store((vad.threshold_db() * 100.0) as i32, Ordering::Relaxed);
                        }
                        timestamp = timestamp.wrapping_add(FRAME_SAMPLES);
                        if !d.send {
                            continue;
                        }
                        let opus = enc.encode(&frame)?;
                        if Encoder::is_dtx_packet(opus) {
                            continue;
                        }
                        seq = seq.wrapping_add(1);
                        let p = AudioPacket {
                            sender_id: 0,
                            seq,
                            timestamp,
                            flags: if d.talk_start { flags::TALK_START } else { 0 },
                            opus,
                        };
                        p.write_to(&mut pkt_buf)?;
                        sp.fetch_add(1, Ordering::Relaxed);
                        let _ = tx_audio_out.send(Bytes::copy_from_slice(&pkt_buf));
                    }
                }

                // 2) Alım: ağ → gönderen başına kuyruk
                while let Ok(bytes) = rx_audio_in.try_recv() {
                    let Ok(p) = AudioPacket::parse(&bytes) else {
                        continue;
                    };
                    rp.fetch_add(1, Ordering::Relaxed);
                    let entry = decoders.entry(p.sender_id).or_insert_with(|| {
                        tracing::info!(sender = p.sender_id, "ilk ses paketi");
                        (
                            Decoder::new().expect("decoder"),
                            VecDeque::new(),
                            Instant::now(),
                        )
                    });
                    let mut decoded = vec![0f32; FRAME];
                    if entry.0.decode(Some(p.opus), &mut decoded).is_ok() {
                        if entry.1.len() >= MAX_QUEUE {
                            entry.1.pop_front();
                        }
                        entry.1.push_back(decoded);
                    }
                    entry.2 = Instant::now();
                }

                // 3) Karıştırma: çıkış tamponu azaldıysa bir 20 ms çerçeve üret
                if out_prod.occupied_len() < out_low_water {
                    mix.iter_mut().for_each(|s| *s = 0.0);
                    for (_, queue, _) in decoders.values_mut() {
                        // Kuyruk MIN_QUEUE'ya ulaşana kadar bekle (mini jitter tamponu); sonra her tikte bir çerçeve.
                        if queue.len() >= MIN_QUEUE {
                            if let Some(f) = queue.pop_front() {
                                for (m, s) in mix.iter_mut().zip(f.iter()) {
                                    *m += s;
                                }
                            }
                        }
                    }
                    // Yumuşak sınırlayıcı: tanh benzeri, 1.0 üstünü ezer.
                    for s in mix.iter_mut() {
                        if s.abs() > 0.9 {
                            *s = s.signum() * (0.9 + (s.abs() - 0.9).tanh() * 0.1);
                        }
                    }
                    out.clear();
                    down.process(&mix, &mut out)?;
                    for &s in &out {
                        let _ = out_prod.try_push(s);
                    }
                    // Uzun süre paket gelmeyen göndereni unut.
                    decoders.retain(|_, (_, _, last)| last.elapsed() < Duration::from_secs(30));
                } else {
                    std::thread::sleep(Duration::from_millis(2));
                }
            }
        })?;

    in_stream.play()?;
    out_stream.play()?;

    // --- Ağ görevleri ---
    let conn = client.connection();
    let send_task = tokio::spawn(async move {
        while let Some(pkt) = rx_audio_out.recv().await {
            let _ = conn.send_datagram(pkt);
        }
    });
    let conn = client.connection();
    let recv_task = tokio::spawn(async move {
        while let Ok(bytes) = conn.read_datagram().await {
            if tx_audio_in.send(bytes).is_err() {
                break;
            }
        }
    });

    tracing::info!(my_id, "odadasın; konuş. Ctrl+C ile çık");
    let mut stats = tokio::time::interval(Duration::from_secs(5));
    let mut ping = tokio::time::interval(Duration::from_secs(2));
    loop {
        tokio::select! {
            msg = client.control().recv() => match msg? {
                ServerMsg::RoomState { room, members } => {
                    let names: Vec<_> = members.iter().map(|m| format!("{}#{}", m.nickname, m.user_id)).collect();
                    tracing::info!(room = room.name, ?names, "oda durumu");
                }
                ServerMsg::MemberJoined(m) => tracing::info!(nick = m.nickname, id = m.user_id, "katıldı"),
                ServerMsg::MemberLeft(id) => tracing::info!(id, "ayrıldı"),
                ServerMsg::MemberUpdated { user_id, muted, deafened } => tracing::info!(user_id, muted, deafened, "güncellendi"),
                ServerMsg::Pong(_) => {}
                ServerMsg::Error { code, message } => tracing::warn!(?code, message, "sunucu hatası"),
                other => tracing::debug!(?other, "mesaj"),
            },
            _ = ping.tick() => { client.control().send(&ClientMsg::Ping(0)).await?; }
            _ = stats.tick() => {
                tracing::info!(
                    rtt_ms = client.rtt().as_millis(),
                    sent = sent_pkts.swap(0, Ordering::Relaxed),
                    recv = recv_pkts.swap(0, Ordering::Relaxed),
                    underruns = underruns.load(Ordering::Relaxed),
                    mic_peak_db = format!("{:.1}", mic_peak_db.swap(-10000, Ordering::Relaxed) as f32 / 100.0),
                    vad_open_pct = vad_open.swap(0, Ordering::Relaxed) * 100 / 250,
                    threshold_db = format!("{:.1}", vad_threshold.load(Ordering::Relaxed) as f32 / 100.0),
                    "istatistik (5 sn)"
                );
            }
            _ = tokio::signal::ctrl_c() => break,
        }
    }

    client.control().send(&ClientMsg::LeaveRoom).await.ok();
    client.close();
    send_task.abort();
    recv_task.abort();
    Ok(())
}
