use statrs::distribution::{Beta, ContinuousCDF, LogNormal, Normal};

use super::{Distribution, estimate::DistributionKind};

impl Distribution {
    pub(super) fn inverse_cdf(&self, probability: f64) -> f64 {
        let probability = probability.clamp(f64::EPSILON, 1.0 - f64::EPSILON);
        match self.0 {
            DistributionKind::Point { value } => value,
            DistributionKind::Normal {
                mean,
                standard_deviation,
            } => Normal::new(mean, standard_deviation)
                .expect("validated normal")
                .inverse_cdf(probability),
            DistributionKind::LogNormal { location, scale } => LogNormal::new(location, scale)
                .expect("validated log-normal")
                .inverse_cdf(probability),
            DistributionKind::Beta { alpha, beta } => Beta::new(alpha, beta)
                .expect("validated beta")
                .inverse_cdf(probability),
            DistributionKind::ScaledBeta {
                alpha,
                beta,
                lower,
                upper,
            } => {
                lower
                    + (upper - lower)
                        * Beta::new(alpha, beta)
                            .expect("validated beta")
                            .inverse_cdf(probability)
            }
            DistributionKind::Empirical { ref samples } => empirical_quantile(samples, probability),
        }
    }
}

fn empirical_quantile(samples: &[f64], probability: f64) -> f64 {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let position = probability * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    sorted[lower] + (sorted[upper] - sorted[lower]) * position.fract()
}
