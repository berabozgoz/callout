//! Basit enerji tabanlı konuşma algılama (VAD).
//!
//! Opus DTX arka plan uğultusunu "ses" sayar; bu VAD kodekten önce durur ve
//! eşiğin altındaki çerçeveleri hiç göndermez. Kelimeler kesilmesin diye:
//! - **ön tampon** (`preroll`): kapı açılınca önceki birkaç çerçeve de gönderilir
//!   (kelime başındaki sessiz ünsüzler: s, ş, h, f);
//! - **histerezis**: açmak için eşik, açık kalmak için eşiğin `HYSTERESIS_DB` altı yeter
//!   (kısık söylenen kelimeler konuşmanın ortasında kesilmez);
//! - **tutma** (`hangover`): konuşma bitince bir süre daha gönderilir (kelime sonları).
//!
//! Gürültü bastırma açıksa VAD temizlenmiş çerçeveye bakar ve RNNoise'in konuşma olasılığını
//! da kullanır (`process_with_prob`): enerji eşiği aşılmasa da olasılık yüksekse kapı açılır.
//! Bas-konuş bunu tamamen atlar.

use crate::codec::FRAME;

/// Açık kapı, açma eşiğinin bu kadar altına inene dek açık kalır.
pub const HYSTERESIS_DB: f32 = 4.0;
/// Uyarlanabilir modda kapatma eşiği gürültü tabanının en az bu kadar üstünde kalır.
const MIN_CLOSE_ABOVE_FLOOR_DB: f32 = 3.0;
/// RNNoise konuşma olasılığı: bunun üstü kapıyı açar, açık kapı `PROB_HOLD` üstünde açık kalır.
const PROB_OPEN: f32 = 0.85;
const PROB_HOLD: f32 = 0.5;

pub struct Vad {
    /// Açma eşiği (doğrusal RMS); kapatma eşiği bunun `HYSTERESIS_DB` altı.
    threshold: f32,
    hangover_frames: u32,
    preroll_frames: u32,
    remaining: u32,
    speaking: bool,
    /// Uyarlanabilir mod: (gürültü tabanı dB, pay dB). Konuşma dışı çerçevelerde
    /// taban yavaşça güncellenir, eşik = taban + pay.
    adaptive: Option<(f32, f32)>,
}

impl Vad {
    /// `threshold_db`: dBFS cinsinden RMS eşiği (ör. -45.0). `hangover_ms`: konuşma
    /// sonrası açık kalma süresi. `preroll_ms`: kapı açılınca geriye dönük gönderilecek süre.
    pub fn new(threshold_db: f32, hangover_ms: u32, preroll_ms: u32) -> Self {
        Self {
            threshold: 10f32.powf(threshold_db / 20.0),
            hangover_frames: hangover_ms / 20,
            preroll_frames: preroll_ms / 20,
            remaining: 0,
            speaking: false,
            adaptive: None,
        }
    }

    /// Uyarlanabilir eşik: `floor_db` başlangıç gürültü tabanı, `margin_db` üstüne pay.
    pub fn set_adaptive(&mut self, floor_db: f32, margin_db: f32) {
        self.adaptive = Some((floor_db, margin_db));
        self.set_threshold_db((floor_db + margin_db).clamp(-60.0, -20.0));
    }

    pub fn noise_floor_db(&self) -> Option<f32> {
        self.adaptive.map(|(f, _)| f)
    }

    /// Çerçeve RMS'i (0.0–1.0).
    pub fn rms(frame: &[f32]) -> f32 {
        (frame.iter().map(|s| s * s).sum::<f32>() / FRAME as f32).sqrt()
    }

    /// Bu çerçeve gönderilmeli mi? Ayrıca konuşma başlangıcını döner
    /// (`AudioPacket` `TALK_START` bayrağı için).
    pub fn process(&mut self, frame: &[f32]) -> VadDecision {
        self.process_with_prob(frame, None)
    }

    /// [`Vad::process`] + gürültü bastırıcının konuşma olasılığı (0.0–1.0).
    pub fn process_with_prob(&mut self, frame: &[f32], speech_prob: Option<f32>) -> VadDecision {
        let rms = Self::rms(frame);
        // Kapatma eşiği gürültü tabanına fazla yaklaşmasın (dalgalanan gürültü kapıyı açık tutmasın).
        let mut close = self.threshold * 10f32.powf(-HYSTERESIS_DB / 20.0);
        if let Some((floor, _)) = self.adaptive {
            close = close.max(10f32.powf((floor + MIN_CLOSE_ABOVE_FLOOR_DB) / 20.0));
        }
        let energy = rms >= if self.speaking { close } else { self.threshold };
        let prob = speech_prob.unwrap_or(0.0);
        let active = energy || prob >= if self.speaking { PROB_HOLD } else { PROB_OPEN };
        if let Some((floor, margin)) = self.adaptive.as_mut() {
            if !active && !self.speaking {
                // Taban: asimetrik EMA, yalnızca konuşma dışında (tutma süresi dahil değil:
                // eşiğin altında kalan kısık heceler tabanı konuşmaya doğru tırmandırmasın).
                // Aşağı hızlı (ortam sakinleşince hemen takip et), yukarı yavaş.
                let db = Self::to_db(rms);
                let alpha = if db < *floor { 0.05 } else { 0.005 };
                *floor += (db - *floor) * alpha;
                self.threshold = 10f32.powf((*floor + *margin).clamp(-60.0, -20.0) / 20.0);
            }
        }
        let was_speaking = self.speaking;
        if active {
            self.remaining = self.hangover_frames;
            self.speaking = true;
        } else if self.remaining > 0 {
            self.remaining -= 1;
        } else {
            self.speaking = false;
        }
        let talk_start = self.speaking && !was_speaking;
        VadDecision {
            send: self.speaking,
            talk_start,
            preroll: if talk_start { self.preroll_frames } else { 0 },
        }
    }

    pub fn is_speaking(&self) -> bool {
        self.speaking
    }

    pub fn set_threshold_db(&mut self, db: f32) {
        self.threshold = 10f32.powf(db / 20.0);
    }

    pub fn threshold_db(&self) -> f32 {
        20.0 * self.threshold.log10()
    }

    /// Doğrusal RMS → dBFS (sessizlik için -100 tabanı).
    pub fn to_db(rms: f32) -> f32 {
        20.0 * rms.max(1e-5).log10()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VadDecision {
    pub send: bool,
    pub talk_start: bool,
    /// Konuşma başladıysa: bu çerçeveden önceki kaç çerçeve de gönderilmeli (ön tampon).
    pub preroll: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gates_silence_and_keeps_hangover() {
        let mut vad = Vad::new(-40.0, 60, 0); // 3 çerçeve hangover
        let silence = vec![0.0005f32; FRAME]; // ≈ -66 dBFS
        let voice = vec![0.1f32; FRAME]; // -20 dBFS
        assert!(!vad.process(&silence).send);
        let d = vad.process(&voice);
        assert!(d.send && d.talk_start);
        assert!(!vad.process(&voice).talk_start);
        assert!(vad.process(&silence).send);
        assert!(vad.process(&silence).send);
        assert!(vad.process(&silence).send);
        assert!(!vad.process(&silence).send);
    }

    #[test]
    fn preroll_only_at_talk_start_and_hysteresis_holds() {
        let mut vad = Vad::new(-40.0, 0, 60);
        let quiet = vec![0.0005f32; FRAME]; // ≈ -66 dBFS
        let loud = vec![0.02f32; FRAME]; // ≈ -34 dBFS
        let soft = vec![0.008f32; FRAME]; // ≈ -42 dBFS: açmaz, ama açık kapıyı tutar
        assert_eq!(vad.process(&soft).preroll, 0);
        assert!(!vad.process(&soft).send);
        let d = vad.process(&loud);
        assert!(d.talk_start);
        assert_eq!(d.preroll, 3);
        assert_eq!(vad.process(&loud).preroll, 0);
        assert!(
            vad.process(&soft).send,
            "histerezis: eşiğin 4 dB altına kadar açık"
        );
        assert!(!vad.process(&quiet).send);
    }

    #[test]
    fn speech_probability_opens_and_holds_gate() {
        let mut vad = Vad::new(-40.0, 0, 0);
        let quiet = vec![0.0005f32; FRAME]; // eşiğin çok altında
        assert!(
            !vad.process_with_prob(&quiet, Some(0.6)).send,
            "0.6 açmaya yetmez"
        );
        assert!(vad.process_with_prob(&quiet, Some(0.9)).talk_start);
        assert!(
            vad.process_with_prob(&quiet, Some(0.6)).send,
            "açık kapıyı tutar"
        );
        assert!(!vad.process_with_prob(&quiet, Some(0.2)).send);
    }
}

#[cfg(test)]
mod sim {
    use super::*;
    use std::f32::consts::TAU;

    /// Konuşma benzeri çerçeve dizisi: (çerçeve, konuşma mı). Gürültü tabanı `noise_db`.
    pub fn speech(noise_db: f32, seconds: usize) -> Vec<(Vec<f32>, bool)> {
        let mut x: u64 = 12345;
        let mut rnd = move || {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((x >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
        };
        let noise_amp = 10f32.powf(noise_db / 20.0) * 3f32.sqrt(); // düzgün dağılım RMS düzeltmesi
        let syllable = [
            -45.0, -38.0, -32.0, -27.0, -26.0, -26.0, -28.0, -31.0, -36.0, -42.0,
        ];
        let mut env: Vec<Option<f32>> = Vec::new();
        let mut word = 0;
        while env.len() < seconds * 50 {
            for _ in 0..4 {
                // Her üçüncü kelime kısık; kelimeler sessiz bir ünsüzle (s, ş, h) başlar.
                let soft = if word % 3 == 2 { -12.0 } else { 0.0 };
                env.extend([Some(-46.0 + soft / 2.0), Some(-43.0 + soft / 2.0)]);
                for _ in 0..2 {
                    env.extend(syllable.iter().map(|d| Some(d + soft)));
                    env.extend([None; 2]);
                }
                env.extend([None; 5]);
                word += 1;
            }
            env.extend([None; 40]);
        }
        let mut phase = 0f32;
        env.into_iter()
            .map(|e| {
                let amp = e.map(|d| 10f32.powf(d / 20.0) * 2f32.sqrt()).unwrap_or(0.0);
                let f: Vec<f32> = (0..FRAME)
                    .map(|_| {
                        phase += TAU * 160.0 / 48_000.0;
                        amp * phase.sin() + noise_amp * rnd()
                    })
                    .collect();
                (f, e.is_some())
            })
            .collect()
    }

    /// Motorun yaptığı gibi: 75 çerçeve kalibrasyon (medyan), sonra uyarlanabilir VAD.
    /// Dönüş: (konuşma çerçevelerinin gönderilen oranı, konuşma dışı çerçevelerin gönderilen oranı).
    pub fn capture(noise_db: f32, hangover_ms: u32, preroll_ms: u32) -> (f32, f32) {
        let frames = speech(noise_db, 60);
        let mut vad = Vad::new(-45.0, hangover_ms, preroll_ms);
        // Kalibrasyon sessizlikte yapılır (uygulama açılırken).
        let mut calib: Vec<f32> = frames
            .iter()
            .filter(|f| !f.1)
            .take(75)
            .map(|f| Vad::to_db(Vad::rms(&f.0)))
            .collect();
        calib.sort_by(|a, b| a.partial_cmp(b).unwrap());
        vad.set_adaptive(calib[calib.len() / 2], 6.0);
        let mut sent = vec![false; frames.len()];
        for (i, (f, _)) in frames.iter().enumerate() {
            let d = vad.process(f);
            if d.send {
                sent[i] = true;
                for j in 0..d.preroll as usize {
                    if i > j {
                        sent[i - 1 - j] = true;
                    }
                }
            }
        }
        let ratio = |speech: bool| {
            let idx: Vec<usize> = (0..frames.len())
                .filter(|&i| frames[i].1 == speech)
                .collect();
            idx.iter().filter(|&&i| sent[i]).count() as f32 / idx.len() as f32
        };
        (ratio(true), ratio(false))
    }

    /// Gürültülü mikrofonda (−42 dB taban; ilk sürümde konuşmanın %68'i gidiyordu)
    /// kısık kelimeler ve kelime başları gitmeli, gürültü kapıyı açık tutmamalı.
    #[test]
    fn keeps_soft_speech_without_locking_open() {
        for (noise, min_speech) in [(-60.0, 0.99), (-50.0, 0.99), (-42.0, 0.9)] {
            let (speech, other) = capture(noise, 400, 60);
            assert!(
                speech >= min_speech,
                "taban {noise}: konuşma %{:.1}",
                speech * 100.0
            );
            assert!(
                other < 0.75,
                "taban {noise}: konuşma dışı %{:.1}",
                other * 100.0
            );
        }
    }

    #[test]
    #[ignore = "deney: VAD ayar taraması"]
    fn report() {
        for n in [-60.0, -50.0, -42.0] {
            for (h, p) in [(300, 0), (300, 60), (400, 60), (500, 60)] {
                let (a, b) = capture(n, h, p);
                println!(
                    "taban {n} dB, tutma {h} ms, ön tampon {p} ms: konuşma %{:.1}, konuşma dışı %{:.1}",
                    a * 100.0,
                    b * 100.0
                );
            }
        }
    }
}
