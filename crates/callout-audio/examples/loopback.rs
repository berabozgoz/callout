//! Mikrofon → hoparlör yerel döngü. Faz 1 adım 1'in ölçüm aracı.
//!
//! Çalıştır: `cargo run -p callout-audio --example loopback`
//! Kulaklıkla dinleyin; hoparlörle çalıştırırsanız geri besleme olur.
//!
//! Gerçek zamanlı kural burada da geçerli: iki geri çağrı yalnızca halka
//! tamponla konuşur, tahsis ve kilit yoktur.

use anyhow::{Context, Result};
use callout_audio::{find_device, list_devices, Direction};
use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    for d in list_devices()? {
        tracing::info!(?d.direction, default = d.is_default, rate = d.sample_rate_hz, ch = d.channels, "{}", d.name);
    }

    let input = find_device(Direction::Input, None).context("giriş cihazı")?;
    let output = find_device(Direction::Output, None).context("çıkış cihazı")?;

    let in_cfg = input.default_input_config()?;
    let out_cfg = output.default_output_config()?;
    tracing::info!(?in_cfg, ?out_cfg, "yapılandırma");

    // Faz 0 basitleştirmesi: iki cihazın örnekleme hızı ve kanal sayısı aynı kabul edilir.
    // Faz 1'de araya `rubato` ile yeniden örnekleme girer.
    anyhow::ensure!(
        in_cfg.sample_rate() == out_cfg.sample_rate(),
        "giriş ve çıkış örnekleme hızı farklı ({} vs {}); Faz 1'de yeniden örnekleme eklenecek",
        in_cfg.sample_rate().0,
        out_cfg.sample_rate().0
    );

    let in_ch = in_cfg.channels() as usize;
    let out_ch = out_cfg.channels() as usize;

    // 200 ms'lik mono tampon: gecikme ölçümü için bilerek küçük.
    let capacity = (in_cfg.sample_rate().0 as usize) / 5;
    let rb = HeapRb::<f32>::new(capacity);
    let (mut prod, mut cons) = rb.split();

    let overruns = Arc::new(AtomicU64::new(0));
    let underruns = Arc::new(AtomicU64::new(0));
    let (ov, un) = (overruns.clone(), underruns.clone());

    let in_stream = input.build_input_stream(
        &in_cfg.clone().into(),
        move |data: &[f32], _| {
            // Çok kanallı girişi monoya indir, tampona yaz.
            for frame in data.chunks(in_ch) {
                let mono = frame.iter().sum::<f32>() / in_ch as f32;
                if prod.try_push(mono).is_err() {
                    ov.fetch_add(1, Ordering::Relaxed);
                }
            }
        },
        |e| tracing::error!("giriş hatası: {e}"),
        None,
    )?;

    let out_stream = output.build_output_stream(
        &out_cfg.clone().into(),
        move |data: &mut [f32], _| {
            for frame in data.chunks_mut(out_ch) {
                let s = cons.try_pop().unwrap_or_else(|| {
                    un.fetch_add(1, Ordering::Relaxed);
                    0.0
                });
                frame.fill(s);
            }
        },
        |e| tracing::error!("çıkış hatası: {e}"),
        None,
    )?;

    in_stream.play()?;
    out_stream.play()?;
    tracing::info!("yerel döngü çalışıyor; Ctrl+C ile çıkın");

    loop {
        std::thread::sleep(Duration::from_secs(5));
        tracing::info!(
            overruns = overruns.load(Ordering::Relaxed),
            underruns = underruns.load(Ordering::Relaxed),
            "tampon istatistiği"
        );
    }
}
