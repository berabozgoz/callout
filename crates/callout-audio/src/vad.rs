//! Basit enerji tabanlı konuşma algılama (VAD).
//!
//! Opus DTX arka plan uğultusunu "ses" sayar; bu VAD kodekten önce durur ve
//! eşiğin altındaki çerçeveleri hiç göndermez. Konuşma bitince `hangover`
//! kadar çerçeve daha gönderilir ki kelime sonları kesilmesin.
//! Faz 2'de gürültü bastırma bunun önüne girer; bas-konuş ise bunu tamamen atlar.

use crate::codec::FRAME;

pub struct Vad {
    threshold: f32,
    hangover_frames: u32,
    remaining: u32,
    speaking: bool,
    /// Uyarlanabilir mod: (gürültü tabanı dB, pay dB). Konuşma dışı çerçevelerde
    /// taban yavaşça güncellenir, eşik = taban + pay.
    adaptive: Option<(f32, f32)>,
}

impl Vad {
    /// `threshold_db`: dBFS cinsinden RMS eşiği (ör. -45.0). `hangover_ms`: konuşma
    /// sonrası açık kalma süresi.
    pub fn new(threshold_db: f32, hangover_ms: u32) -> Self {
        Self {
            threshold: 10f32.powf(threshold_db / 20.0),
            hangover_frames: hangover_ms / 20,
            remaining: 0,
            speaking: false,
            adaptive: None,
        }
    }

    /// Uyarlanabilir eşik: `floor_db` başlangıç gürültü tabanı, `margin_db` üstüne pay.
    pub fn set_adaptive(&mut self, floor_db: f32, margin_db: f32) {
        self.adaptive = Some((floor_db, margin_db));
        self.set_threshold_db(floor_db + margin_db);
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
        let rms = Self::rms(frame);
        let active = rms >= self.threshold;
        if let Some((floor, margin)) = self.adaptive.as_mut() {
            if !active {
                // Taban: yavaş EMA (~1 s zaman sabiti), yalnızca sessiz çerçevelerde.
                let db = Self::to_db(rms);
                *floor += (db - *floor) * 0.02;
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
        VadDecision {
            send: self.speaking,
            talk_start: self.speaking && !was_speaking,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gates_silence_and_keeps_hangover() {
        let mut vad = Vad::new(-40.0, 60); // 3 çerçeve hangover
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
}
