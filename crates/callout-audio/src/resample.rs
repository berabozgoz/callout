//! Cihaz örnekleme hızı ↔ 48 kHz dönüşümü (rubato, FFT tabanlı, mono).
//!
//! Gerçek zamanlı iş parçacığında **değil**, ses iş parçacığında çalışır;
//! içeride tampon tutar, bu yüzden girdi uzunluğu serbesttir.

use rubato::{FftFixedInOut, Resampler as _};

#[derive(Debug, thiserror::Error)]
pub enum ResampleError {
    #[error("yeniden örnekleyici kurulamadı: {0}")]
    Construct(#[from] rubato::ResamplerConstructionError),
    #[error("yeniden örnekleme hatası: {0}")]
    Process(#[from] rubato::ResampleError),
}

pub struct Resampler {
    inner: Option<FftFixedInOut<f32>>,
    chunk_in: usize,
    pending_in: Vec<f32>,
    in_buf: Vec<Vec<f32>>,
    out_buf: Vec<Vec<f32>>,
}

impl Resampler {
    /// `from_hz == to_hz` ise kimlik dönüşümü (gecikmesiz).
    pub fn new(from_hz: u32, to_hz: u32) -> Result<Self, ResampleError> {
        if from_hz == to_hz {
            return Ok(Self {
                inner: None,
                chunk_in: 0,
                pending_in: Vec::new(),
                in_buf: Vec::new(),
                out_buf: Vec::new(),
            });
        }
        // ~10 ms'lik giriş parçası hedeflenir; rubato bunu gcd'ye göre yuvarlar.
        let inner =
            FftFixedInOut::<f32>::new(from_hz as usize, to_hz as usize, from_hz as usize / 100, 1)?;
        let chunk_in = inner.input_frames_next();
        let out_max = inner.output_frames_max();
        Ok(Self {
            inner: Some(inner),
            chunk_in,
            pending_in: Vec::with_capacity(chunk_in * 2),
            in_buf: vec![vec![0f32; chunk_in]],
            out_buf: vec![vec![0f32; out_max]],
        })
    }

    /// Dönüşümün eklediği gecikme (çıkış örneği cinsinden).
    pub fn delay(&self) -> usize {
        self.inner.as_ref().map(|r| r.output_delay()).unwrap_or(0)
    }

    /// `input`'u işler, dönüştürülmüş örnekleri `out`'a **ekler**.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<(), ResampleError> {
        let Some(inner) = self.inner.as_mut() else {
            out.extend_from_slice(input);
            return Ok(());
        };
        self.pending_in.extend_from_slice(input);
        let mut consumed = 0;
        while self.pending_in.len() - consumed >= self.chunk_in {
            self.in_buf[0].copy_from_slice(&self.pending_in[consumed..consumed + self.chunk_in]);
            let (_, n_out) = inner.process_into_buffer(&self.in_buf, &mut self.out_buf, None)?;
            out.extend_from_slice(&self.out_buf[0][..n_out]);
            consumed += self.chunk_in;
        }
        self.pending_in.drain(..consumed);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_passthrough() {
        let mut r = Resampler::new(48_000, 48_000).unwrap();
        let mut out = Vec::new();
        r.process(&[1.0, 2.0, 3.0], &mut out).unwrap();
        assert_eq!(out, vec![1.0, 2.0, 3.0]);
        assert_eq!(r.delay(), 0);
    }

    #[test]
    fn converts_44100_to_48000_with_right_ratio() {
        let mut r = Resampler::new(44_100, 48_000).unwrap();
        let mut out = Vec::new();
        let input: Vec<f32> = (0..44_100).map(|i| (i as f32 * 0.01).sin()).collect();
        // Arbitrary chunk sizes to exercise the internal FIFO.
        for chunk in input.chunks(333) {
            r.process(chunk, &mut out).unwrap();
        }
        // Bir saniyelik giriş ≈ 48_000 çıkış (iç tamponda kalan parça hariç).
        assert!(
            out.len() > 46_000 && out.len() <= 48_000,
            "çıkış {}",
            out.len()
        );
        assert!(r.delay() > 0);
    }
}
