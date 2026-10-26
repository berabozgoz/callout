//! Gönderen başına jitter tamponu.
//!
//! Paketler `seq` ile sıralanır; her 20 ms'lik çalma tikinde [`JitterBuffer::pop`]
//! çağrılır ve üç sonuçtan biri döner: paket, kayıp (PLC/FEC ile doldur) ya da
//! boşta (gönderen susmuş, sessizlik). Hedef derinlik (`target`) prefill
//! süresidir: ilk paketten sonra tampon `target` çerçeveye ulaşana kadar
//! çalma başlamaz. Geç gelen (zaten atlanmış) paketler ve tampon boşalmaları
//! sayılır; 5 saniyelik pencerede bunlar artarsa hedef büyür, uzun süre sıfır
//! kalırsa küçülür.
//!
//! Sabit kapasiteli ve tahsisi az: paketler `Vec<u8>` olarak saklanır, bu
//! modül ses iş parçacığında (gerçek zamanlı geri çağrıda değil) çalışır.

use std::collections::BTreeMap;

/// Çalma tikinde tamponun kararı.
#[derive(Debug, PartialEq, Eq)]
pub enum Playout {
    /// Sıradaki paket; Opus yükü.
    Packet(Vec<u8>),
    /// Sıradaki paket kayıp. `next` varsa bir sonraki paket elde; `decode_fec`
    /// ile kayıp çerçeve kurtarılabilir, yoksa PLC (`decode(None)`).
    Lost { next: Option<Vec<u8>> },
    /// Gönderen susmuş ya da henüz prefill dolmadı; sessizlik üret.
    Idle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JitterStats {
    pub late: u32,
    pub lost: u32,
    pub fec_recovered: u32,
    pub underflow: u32,
    pub target: u32,
}

pub struct JitterBuffer {
    packets: BTreeMap<u64, Vec<u8>>,
    /// Sarmalanmamış sıra sayacı; `None` = boşta.
    next: Option<u64>,
    last_unwrapped: Option<u64>,
    prefilling: bool,
    misses: u32,
    target: u32,
    min_target: u32,
    max_target: u32,
    window_late: u32,
    window_underflow: u32,
    quiet_windows: u32,
    stats: JitterStats,
}

/// Peş peşe bu kadar tikte paket yoksa gönderen susmuş sayılır (PLC durur).
const IDLE_AFTER_MISSES: u32 = 3;
/// Tampon bu kadar çerçeveyi aşarsa (hedef + pay) en eskiler atılır.
const OVERFLOW_MARGIN: u32 = 8;

impl JitterBuffer {
    /// `target_frames`: başlangıç prefill (20 ms birimleri; 3 = 60 ms).
    pub fn new(target_frames: u32) -> Self {
        let target = target_frames.clamp(1, 20);
        Self {
            packets: BTreeMap::new(),
            next: None,
            last_unwrapped: None,
            prefilling: true,
            misses: 0,
            target,
            min_target: 2,
            max_target: 12,
            window_late: 0,
            window_underflow: 0,
            quiet_windows: 0,
            stats: JitterStats {
                target,
                ..Default::default()
            },
        }
    }

    fn unwrap_seq(&mut self, seq: u16) -> u64 {
        let Some(last) = self.last_unwrapped else {
            self.last_unwrapped = Some(seq as u64 + 65536); // negatif farklar için pay
            return self.last_unwrapped.unwrap();
        };
        let last_lo = (last & 0xFFFF) as u16;
        let diff = seq.wrapping_sub(last_lo) as i16 as i64;
        let unwrapped = (last as i64 + diff).max(0) as u64;
        self.last_unwrapped = Some(unwrapped);
        unwrapped
    }

    /// Ağdan gelen paket. `talk_start` (TALK_START bayrağı) yeni konuşma
    /// başlangıcını işaretler; tampon sıfırlanıp yeniden prefill yapılır.
    pub fn push(&mut self, seq: u16, talk_start: bool, opus: &[u8]) {
        if talk_start || self.next.is_none() {
            self.packets.clear();
            self.last_unwrapped = None;
            let u = self.unwrap_seq(seq);
            self.next = Some(u);
            self.prefilling = true;
            self.misses = 0;
            self.packets.insert(u, opus.to_vec());
            return;
        }
        let u = self.unwrap_seq(seq);
        let next = self.next.unwrap();
        if u < next {
            // Çalma sırası geçmiş: geç paket.
            self.stats.late += 1;
            self.window_late += 1;
            return;
        }
        self.packets.insert(u, opus.to_vec());
        // Taşma: hedefin çok üstünde biriktiyse eskileri atla (gecikmeyi sınırla).
        let limit = (self.target + OVERFLOW_MARGIN) as usize;
        while self.packets.len() > limit {
            if let Some((&oldest, _)) = self.packets.iter().next() {
                self.packets.remove(&oldest);
                self.next = Some(oldest + 1);
            }
        }
    }

    /// Her 20 ms çalma tikinde çağrılır.
    pub fn pop(&mut self) -> Playout {
        let Some(next) = self.next else {
            return Playout::Idle;
        };

        if self.prefilling {
            if (self.packets.len() as u32) < self.target {
                return Playout::Idle;
            }
            self.prefilling = false;
        }

        if let Some(p) = self.packets.remove(&next) {
            self.next = Some(next + 1);
            self.misses = 0;
            return Playout::Packet(p);
        }

        // Sıradaki yok.
        if self.packets.is_empty() {
            self.misses += 1;
            if self.misses >= IDLE_AFTER_MISSES {
                // Konuşma bitti: boşta. Bir sonraki paket yeniden prefill başlatır.
                self.next = None;
                self.misses = 0;
                return Playout::Idle;
            }
            self.stats.underflow += 1;
            self.window_underflow += 1;
            self.next = Some(next + 1);
            return Playout::Lost { next: None };
        }

        // Daha ileri bir paket var: bu çerçeve gerçekten kayıp.
        self.stats.lost += 1;
        self.next = Some(next + 1);
        let fec_src = self.packets.get(&(next + 1)).cloned();
        if fec_src.is_some() {
            self.stats.fec_recovered += 1;
        }
        Playout::Lost { next: fec_src }
    }

    /// 5 saniyede bir çağrılır: hedef derinliği pencere istatistiğine göre ayarlar.
    pub fn adapt(&mut self) {
        if self.window_late + self.window_underflow >= 3 {
            self.target = (self.target + 1).min(self.max_target);
            self.quiet_windows = 0;
        } else if self.window_late + self.window_underflow == 0 {
            self.quiet_windows += 1;
            if self.quiet_windows >= 6 {
                self.target = (self.target - 1).max(self.min_target);
                self.quiet_windows = 0;
            }
        }
        self.window_late = 0;
        self.window_underflow = 0;
        self.stats.target = self.target;
    }

    pub fn stats(&self) -> JitterStats {
        self.stats
    }

    pub fn target(&self) -> u32 {
        self.target
    }

    pub fn is_idle(&self) -> bool {
        self.next.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkt(n: u8) -> Vec<u8> {
        vec![n; 4]
    }

    #[test]
    fn prefill_then_in_order() {
        let mut jb = JitterBuffer::new(2);
        jb.push(10, true, &pkt(10));
        assert_eq!(jb.pop(), Playout::Idle); // prefill: 1 < 2
        jb.push(11, false, &pkt(11));
        assert_eq!(jb.pop(), Playout::Packet(pkt(10)));
        assert_eq!(jb.pop(), Playout::Packet(pkt(11)));
    }

    #[test]
    fn reorder_is_fixed() {
        let mut jb = JitterBuffer::new(2);
        jb.push(1, true, &pkt(1));
        jb.push(3, false, &pkt(3));
        jb.push(2, false, &pkt(2));
        assert_eq!(jb.pop(), Playout::Packet(pkt(1)));
        assert_eq!(jb.pop(), Playout::Packet(pkt(2)));
        assert_eq!(jb.pop(), Playout::Packet(pkt(3)));
    }

    #[test]
    fn loss_offers_fec_source_then_continues() {
        let mut jb = JitterBuffer::new(2);
        jb.push(1, true, &pkt(1));
        jb.push(3, false, &pkt(3)); // 2 kayıp
        assert_eq!(jb.pop(), Playout::Packet(pkt(1)));
        assert_eq!(jb.pop(), Playout::Lost { next: Some(pkt(3)) });
        assert_eq!(jb.pop(), Playout::Packet(pkt(3)));
        assert_eq!(jb.stats().lost, 1);
        assert_eq!(jb.stats().fec_recovered, 1);
    }

    #[test]
    fn goes_idle_after_gap_and_restarts() {
        let mut jb = JitterBuffer::new(1);
        jb.push(5, true, &pkt(5));
        assert_eq!(jb.pop(), Playout::Packet(pkt(5)));
        assert_eq!(jb.pop(), Playout::Lost { next: None }); // 1. kaçırma (PLC)
        assert_eq!(jb.pop(), Playout::Lost { next: None }); // 2.
        assert_eq!(jb.pop(), Playout::Idle); // 3. → boşta
        assert!(jb.is_idle());
        jb.push(40, false, &pkt(40)); // talk_start olmadan da yeniden başlar
        assert_eq!(jb.pop(), Playout::Packet(pkt(40)));
    }

    #[test]
    fn late_packet_counted_and_dropped() {
        let mut jb = JitterBuffer::new(1);
        jb.push(1, true, &pkt(1));
        jb.push(2, false, &pkt(2));
        assert_eq!(jb.pop(), Playout::Packet(pkt(1)));
        assert_eq!(jb.pop(), Playout::Packet(pkt(2)));
        jb.push(1, false, &pkt(1)); // geç
        assert_eq!(jb.stats().late, 1);
        assert_eq!(jb.pop(), Playout::Lost { next: None });
    }

    #[test]
    fn seq_wraps_around() {
        let mut jb = JitterBuffer::new(1);
        jb.push(65534, true, &pkt(1));
        jb.push(65535, false, &pkt(2));
        jb.push(0, false, &pkt(3));
        jb.push(1, false, &pkt(4));
        for n in 1..=4 {
            assert_eq!(jb.pop(), Playout::Packet(pkt(n)));
        }
    }

    #[test]
    fn adapt_grows_on_trouble_and_shrinks_when_quiet() {
        let mut jb = JitterBuffer::new(2);
        jb.push(1, true, &pkt(1));
        jb.push(2, false, &pkt(2));
        jb.pop();
        jb.pop();
        for _ in 0..3 {
            jb.push(1, false, &pkt(1)); // geç ×3
        }
        jb.adapt();
        assert_eq!(jb.target(), 3);
        for _ in 0..6 {
            jb.adapt();
        }
        assert_eq!(jb.target(), 2);
    }
}
