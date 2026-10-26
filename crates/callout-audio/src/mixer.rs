//! Karıştırıcı: kullanıcı başına kazanç, toplama ve yumuşak sınırlayıcı.

use crate::codec::FRAME;
use std::collections::HashMap;

pub struct Mixer {
    gains: HashMap<u32, f32>,
    master: f32,
    acc: [f32; FRAME],
    sources: u32,
}

impl Default for Mixer {
    fn default() -> Self {
        Self::new()
    }
}

impl Mixer {
    pub fn new() -> Self {
        Self {
            gains: HashMap::new(),
            master: 1.0,
            acc: [0.0; FRAME],
            sources: 0,
        }
    }

    /// Kullanıcı kazancı (1.0 = değişmez, 0.0 = sessiz, 2.0 = +6 dB).
    pub fn set_gain(&mut self, user_id: u32, gain: f32) {
        self.gains.insert(user_id, gain.clamp(0.0, 4.0));
    }

    pub fn gain(&self, user_id: u32) -> f32 {
        *self.gains.get(&user_id).unwrap_or(&1.0)
    }

    pub fn set_master(&mut self, gain: f32) {
        self.master = gain.clamp(0.0, 2.0);
    }

    /// Yeni çerçeveye başla.
    pub fn begin(&mut self) {
        self.acc.fill(0.0);
        self.sources = 0;
    }

    /// Bir kullanıcının çözülmüş 960 örneğini ekle.
    pub fn add(&mut self, user_id: u32, frame: &[f32]) {
        let g = self.gain(user_id) * self.master;
        if g == 0.0 {
            return;
        }
        for (a, s) in self.acc.iter_mut().zip(frame.iter()) {
            *a += s * g;
        }
        self.sources += 1;
    }

    /// Karışımı `out`'a yazar (sınırlayıcı uygulanmış). Kaynak yoksa sessizlik.
    pub fn finish(&self, out: &mut [f32]) {
        for (o, &a) in out.iter_mut().zip(self.acc.iter()) {
            *o = soft_limit(a);
        }
    }

    pub fn sources(&self) -> u32 {
        self.sources
    }
}

/// |x| ≤ 0.9 doğrusal, üstü tanh ile 1.0'a yaklaşır; klipleme yerine yumuşak ezme.
#[inline]
pub fn soft_limit(x: f32) -> f32 {
    const KNEE: f32 = 0.9;
    if x.abs() <= KNEE {
        x
    } else {
        x.signum() * (KNEE + (1.0 - KNEE) * ((x.abs() - KNEE) / (1.0 - KNEE)).tanh())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_with_gain_and_limits() {
        let mut m = Mixer::new();
        m.set_gain(2, 0.5);
        let a = vec![0.6f32; FRAME];
        let b = vec![0.8f32; FRAME];
        m.begin();
        m.add(1, &a);
        m.add(2, &b); // 0.6 + 0.4 = 1.0 → sınırlayıcı < 1.0
        let mut out = vec![0f32; FRAME];
        m.finish(&mut out);
        assert!(out[0] > 0.9 && out[0] < 1.0, "{}", out[0]);
        assert_eq!(m.sources(), 2);

        m.set_gain(1, 0.0);
        m.begin();
        m.add(1, &a);
        assert_eq!(m.sources(), 0);
    }

    #[test]
    fn limiter_is_monotonic_and_bounded() {
        let mut prev = -1.0;
        for i in 0..400 {
            let x = -2.0 + i as f32 * 0.01;
            let y = soft_limit(x);
            assert!(y >= prev - 1e-6);
            assert!(y.abs() < 1.0 + 1e-6);
            prev = y;
        }
    }
}
