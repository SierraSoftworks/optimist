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
        }
    }
}
