//! Local loop: microphone → (48 kHz) → Opus encode → Opus decode → speaker.
//! Measurement tool for Phase 1 step 2: the codec's effect on sound and CPU.
//!
//! Run: `cargo run -p callout-audio --example loopback`
//! For a raw (no codec) comparison: `cargo run -p callout-audio --example loopback -- --raw`
//! Listen with headphones; running it on speakers causes feedback.
//!
//! Threads:
//! - cpal input callback: only writes to the ring buffer;
//! - audio thread: resample → frame → encode → decode → resample → ring buffer;
//! - cpal output callback: only reads from the ring buffer.

use anyhow::{Context, Result};
use callout_audio::codec::{Decoder, Encoder, EncoderConfig, FRAME};
use callout_audio::framer::Framer;
use callout_audio::resample::Resampler;
use callout_audio::{find_device, list_devices, Direction};
use callout_proto::SAMPLE_RATE_HZ;
use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let raw = std::env::args().any(|a| a == "--raw");

    for d in list_devices()? {
        tracing::info!(?d.direction, default = d.is_default, rate = d.sample_rate_hz, ch = d.channels, "{}", d.name);
    }

    let input = find_device(Direction::Input, None).context("input device")?;
    let output = find_device(Direction::Output, None).context("output device")?;
    let in_cfg = input.default_input_config()?;
    let out_cfg = output.default_output_config()?;
    let in_rate = in_cfg.sample_rate().0;
    let out_rate = out_cfg.sample_rate().0;
    let in_ch = in_cfg.channels() as usize;
    let out_ch = out_cfg.channels() as usize;
    tracing::info!(in_rate, out_rate, in_ch, out_ch, raw, "config");

    // Input and output ring buffers (mono, at device rate), 200 ms.
    let (mut in_prod, mut in_cons) = HeapRb::<f32>::new(in_rate as usize / 5).split();
    let (mut out_prod, mut out_cons) = HeapRb::<f32>::new(out_rate as usize / 5).split();

    let overruns = Arc::new(AtomicU64::new(0));
    let underruns = Arc::new(AtomicU64::new(0));
    let (ov, un) = (overruns.clone(), underruns.clone());

    let in_stream = input.build_input_stream(
        &in_cfg.clone().into(),
        move |data: &[f32], _| {
            for frame in data.chunks(in_ch) {
                let mono = frame.iter().sum::<f32>() / in_ch as f32;
                if in_prod.try_push(mono).is_err() {
                    ov.fetch_add(1, Ordering::Relaxed);
                }
            }
        },
        |e| tracing::error!("input error: {e}"),
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
        |e| tracing::error!("output error: {e}"),
        None,
    )?;

    // Audio thread: codec pipeline.
    let bytes_sent = Arc::new(AtomicU64::new(0));
    let frames_sent = Arc::new(AtomicU64::new(0));
    let dtx_frames = Arc::new(AtomicU64::new(0));
    let enc_nanos = Arc::new(AtomicU64::new(0));
    let (bs, fs, dx, en) = (
        bytes_sent.clone(),
        frames_sent.clone(),
        dtx_frames.clone(),
        enc_nanos.clone(),
    );

    std::thread::Builder::new()
        .name("callout-audio".into())
        .spawn(move || -> Result<()> {
            let mut up = Resampler::new(in_rate, SAMPLE_RATE_HZ)?;
            let mut down = Resampler::new(SAMPLE_RATE_HZ, out_rate)?;
            let mut framer = Framer::new();
            let mut enc = Encoder::new(EncoderConfig::default())?;
            let mut dec = Decoder::new()?;
            tracing::info!(
                up_delay = up.delay(),
                down_delay = down.delay(),
                "resampling delay (samples)"
            );

            let mut chunk = Vec::with_capacity(1024);
            let mut at48 = Vec::with_capacity(4096);
            let mut frame = [0f32; FRAME];
            let mut decoded = vec![0f32; FRAME];
            let mut out = Vec::with_capacity(4096);

            loop {
                chunk.clear();
                while let Some(s) = in_cons.try_pop() {
                    chunk.push(s);
                    if chunk.len() == 1024 {
                        break;
                    }
                }
                if chunk.is_empty() {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                }

                at48.clear();
                up.process(&chunk, &mut at48)?;
                framer.push(&at48);

                while framer.next_frame(&mut frame) {
                    out.clear();
                    if raw {
                        down.process(&frame, &mut out)?;
                    } else {
                        let t0 = Instant::now();
                        let pkt = enc.encode(&frame)?.to_vec();
                        en.fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
                        fs.fetch_add(1, Ordering::Relaxed);
                        // DTX packets are not sent; the receiver treats them like a "loss" and produces PLC/silence.
                        let packet = if Encoder::is_dtx_packet(&pkt) {
                            dx.fetch_add(1, Ordering::Relaxed);
                            None
                        } else {
                            bs.fetch_add(pkt.len() as u64, Ordering::Relaxed);
                            Some(pkt.as_slice())
                        };
                        dec.decode(packet, &mut decoded)?;
                        down.process(&decoded, &mut out)?;
                    }
                    for &s in &out {
                        let _ = out_prod.try_push(s); // drop on overflow; the underrun counter is on the other side
                    }
                }
            }
        })?;

    in_stream.play()?;
    out_stream.play()?;
    tracing::info!(
        "local loop running ({}); press Ctrl+C to quit",
        if raw { "raw" } else { "Opus" }
    );

    let mut last = Instant::now();
    loop {
        std::thread::sleep(Duration::from_secs(5));
        let secs = last.elapsed().as_secs_f64();
        last = Instant::now();
        let frames = frames_sent.swap(0, Ordering::Relaxed);
        let bytes = bytes_sent.swap(0, Ordering::Relaxed);
        let dtx = dtx_frames.swap(0, Ordering::Relaxed);
        let nanos = enc_nanos.swap(0, Ordering::Relaxed);
        tracing::info!(
            overruns = overruns.load(Ordering::Relaxed),
            underruns = underruns.load(Ordering::Relaxed),
            kbps = format!("{:.1}", bytes as f64 * 8.0 / 1000.0 / secs),
            dtx_pct = format!(
                "{:.0}",
                if frames > 0 {
                    dtx as f64 * 100.0 / frames as f64
                } else {
                    0.0
                }
            ),
            encode_us = nanos.checked_div(frames).unwrap_or(0) / 1000,
            "stats"
        );
    }
}
