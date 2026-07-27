use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;

use super::{Distribution, Kind};

impl Distribution {
    /// Draws one reproducible sample using `seed`.
    pub fn sample_seeded(&self, seed: u64) -> Result<f64, String> {
        self.sample(&mut ChaCha20Rng::seed_from_u64(seed))
    }

    pub(crate) fn sample(&self, rng: &mut ChaCha20Rng) -> Result<f64, String> {
        let probability = rng.gen_range(f64::EPSILON..(1.0 - f64::EPSILON));
        match &self.kind {
            Kind::Samples(samples) => {
                if samples.is_empty() {
                    return Err("cannot sample an empty empirical distribution".into());
                }
                samples
                    .get(rng.gen_range(0..samples.len()))
                    .copied()
                    .ok_or_else(|| "empirical sample index is out of bounds".into())
            }
            _ => self.quantile(probability),
        }
    }

    pub(crate) fn sample_n(&self, count: usize, rng: &mut ChaCha20Rng) -> Result<Vec<f64>, String> {
        (0..count).map(|_| self.sample(rng)).collect()
    }
}
