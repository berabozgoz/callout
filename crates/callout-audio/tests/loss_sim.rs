//! Kayıplı ağ simülasyonu: kodlayıcı → kayıp → çözücü (FEC/PLC).
//!
//! Kayıpsız çözülmüş çıktıyı referans alıp, kayıp çerçevelerde FEC + PLC ile
//! yalnız PLC'yi karşılaştırır. Sonuçları görmek için:
//! `cargo test -p callout-audio --test loss_sim -- --nocapture`

use callout_audio::codec::{Decoder, Encoder, EncoderConfig, FRAME};
use std::f32::consts::TAU;

const SR: f32 = 48_000.0;
const FRAMES: usize = 1500; // 30 sn

/// Konuşmaya benzer sinyal: kayan perdeli harmonikler, hece hızında genlik.
fn voice_like(frames: usize) -> Vec<f32> {
    let mut phase = 0f32;
    (0..frames * FRAME)
        .map(|i| {
            let t = i as f32 / SR;
            let f0 = 140.0 + 40.0 * (TAU * 0.7 * t).sin();
            phase += TAU * f0 / SR;
            let env = 0.55 + 0.45 * (TAU * 4.0 * t).sin();
            let mut s = 0.0;
            for h in 1..=8 {
                s += (phase * h as f32).sin() / h as f32;
            }
            0.25 * env * s
        })
        .collect()
}

/// Belirlenimci kayıp deseni (LCG). `burst`: kayıp başlayınca devam etme olasılığı.
fn loss_pattern(frames: usize, start_p: f64, burst: f64, seed: u64) -> Vec<bool> {
    let mut x = seed;
    let mut rnd = || {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (x >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut lost = vec![false; frames];
    let mut in_burst = false;
    // İlk çerçeveler prefill/ısınma; kayıpsız bırak.
    for l in lost.iter_mut().skip(10) {
        in_burst = if in_burst {
            rnd() < burst
        } else {
            rnd() < start_p
        };
        *l = in_burst;
    }
    lost
}

struct Result {
    loss_pct: f32,
    snr_lost_fec: f32,
    snr_lost_plc: f32,
    fec_recovered: u32,
}

fn snr_db(reference: &[f32], test: &[f32]) -> f32 {
    let sig: f64 = reference.iter().map(|s| (*s as f64).powi(2)).sum();
    let err: f64 = reference
        .iter()
        .zip(test)
        .map(|(a, b)| ((a - b) as f64).powi(2))
        .sum();
    (10.0 * (sig / err.max(1e-12)).log10()) as f32
}

/// Paketleri `lost` desenine göre düşürüp çözer; jitter tamponunun kayıp anındaki
/// kararını taklit eder (tampon yeterince doluyken: sonraki paket eldeyse FEC, yoksa PLC).
/// `use_fec` kapalıysa kayıp çerçeveler yalnız PLC ile doldurulur.
fn play(packets: &[Vec<u8>], lost: &[bool], use_fec: bool) -> (Vec<Vec<f32>>, u32) {
    let mut dec = Decoder::new().unwrap();
    let mut out = Vec::with_capacity(packets.len());
    let mut buf = vec![0f32; FRAME];
    let mut fec = 0;
    for i in 0..packets.len() - 1 {
        if !lost[i] {
            dec.decode(Some(&packets[i]), &mut buf).unwrap();
        } else if use_fec && !lost[i + 1] {
            dec.decode_fec(&packets[i + 1], &mut buf).unwrap();
            fec += 1;
        } else {
            dec.decode(None, &mut buf).unwrap();
        }
        out.push(buf.clone());
    }
    (out, fec)
}

fn run(cfg: EncoderConfig, start_p: f64, burst: f64) -> Result {
    let input = voice_like(FRAMES);
    let mut enc = Encoder::new(cfg).unwrap();
    let packets: Vec<Vec<u8>> = input
        .chunks(FRAME)
        .map(|f| enc.encode(f).unwrap().to_vec())
        .collect();
    let lost = loss_pattern(FRAMES, start_p, burst, 7);
    let none = vec![false; FRAMES];

    let (reference, _) = play(&packets, &none, true);
    let (with_fec, fec_recovered) = play(&packets, &lost, true);
    let (plc_only, _) = play(&packets, &lost, false);

    // Yalnız kayıp çerçeveler (FEC'in kurtarmaya çalıştığı yerler) + sonraki çerçeve
    // (PLC'den sonra çözücü durumu toparlanırken bozulma sürer).
    let mut r = Vec::new();
    let mut f = Vec::new();
    let mut p = Vec::new();
    for i in 0..reference.len() {
        if lost[i] || (i > 0 && lost[i - 1]) {
            r.extend_from_slice(&reference[i]);
            f.extend_from_slice(&with_fec[i]);
            p.extend_from_slice(&plc_only[i]);
        }
    }
    Result {
        loss_pct: 100.0 * lost.iter().filter(|l| **l).count() as f32 / FRAMES as f32,
        snr_lost_fec: snr_db(&r, &f),
        snr_lost_plc: snr_db(&r, &p),
        fec_recovered,
    }
}

#[test]
fn fec_beats_plc_on_random_loss() {
    for (name, start_p, burst) in [
        ("%5 tekil", 0.05, 0.0),
        ("%10 tekil", 0.10, 0.0),
        ("%8 patlama", 0.04, 0.5),
    ] {
        let r = run(EncoderConfig::default(), start_p, burst);
        println!(
            "{name}: kayıp %{:.1}, FEC kurtarılan {}, kayıp çerçevelerde SNR: FEC+PLC {:.1} dB, yalnız PLC {:.1} dB",
            r.loss_pct, r.fec_recovered, r.snr_lost_fec, r.snr_lost_plc
        );
        if burst == 0.0 {
            assert!(
                r.snr_lost_fec > r.snr_lost_plc + 3.0,
                "{name}: FEC belirgin fayda sağlamadı ({:.1} vs {:.1} dB)",
                r.snr_lost_fec,
                r.snr_lost_plc
            );
        }
    }
}

#[test]
fn fec_disabled_gives_no_gain() {
    // Kontrol: kodlayıcıda FEC kapalıyken decode_fec, PLC'den iyi olmamalı.
    let cfg = EncoderConfig {
        fec: false,
        expected_loss_pct: 0,
        ..EncoderConfig::default()
    };
    let r = run(cfg, 0.10, 0.0);
    println!(
        "FEC kapalı: kayıp %{:.1}, SNR FEC yolu {:.1} dB, PLC {:.1} dB",
        r.loss_pct, r.snr_lost_fec, r.snr_lost_plc
    );
    assert!(r.snr_lost_fec < r.snr_lost_plc + 3.0);
}

#[test]
#[ignore = "deney: kayıp varsayımı taraması"]
fn sweep_expected_loss() {
    for pct in [5, 10, 20, 30] {
        let cfg = EncoderConfig {
            expected_loss_pct: pct,
            ..EncoderConfig::default()
        };
        let input = voice_like(FRAMES);
        let mut enc = Encoder::new(cfg).unwrap();
        let bytes: usize = input
            .chunks(FRAME)
            .map(|f| enc.encode(f).unwrap().len())
            .sum();
        let single = run(cfg, 0.10, 0.0);
        let burst = run(cfg, 0.04, 0.5);
        println!(
            "varsayım %{pct}: ort. paket {} bayt, tekil SNR {:.1}/{:.1} dB, patlama SNR {:.1}/{:.1} dB",
            bytes / FRAMES,
            single.snr_lost_fec,
            single.snr_lost_plc,
            burst.snr_lost_fec,
            burst.snr_lost_plc
        );
    }
}
