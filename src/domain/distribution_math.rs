use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rand_distr::{Beta, Distribution as _, LogNormal, Normal};
use serde::Serialize;

use super::Distribution;
use super::estimate::DistributionKind;

/// Exact population moments for a validated primitive distribution.
///
/// `variance` is the second central moment, not a sample estimate. Both values
/// can overflow to infinity for a mathematically valid LogNormal with extreme
/// parameters; callers should check finiteness before numerical analysis.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct DistributionMoments {
    /// Exact population expectation, $E\[X\]$.
    pub mean: f64,
    /// Exact population variance, $E[(X-E\[X\])^2]$.
    pub variance: f64,
}

impl Distribution {
    /// Returns the exact mean and variance of this primitive distribution.
    ///
    /// For $B\sim\operatorname{Beta}(\alpha,\beta)$,
    /// $E\[B\]=\alpha/(\alpha+\beta)$ and
    /// $\operatorname{Var}(B)=\alpha\beta/((\alpha+\beta)^2(\alpha+\beta+1))$.
    /// A ScaledBeta applies the affine transform $l+(u-l)B$. For
    /// $X\sim\operatorname{LogNormal}(\mu,\sigma)$, the implementation uses
    /// $E\[X\]=e^{\mu+\sigma^2/2}$ and
    /// $\operatorname{Var}(X)=(e^{\sigma^2}-1)e^{2\mu+\sigma^2}$.
    /// Normal and point-mass moments follow directly from their constructors.
    /// These are analytical population moments; no independence assumption or
    /// numerical approximation is involved. See NIST/SEMATECH, sections 1.3.6.6,
    /// 1.3.6.7, and 1.3.6.23.
    pub fn moments(&self) -> DistributionMoments {
        match self.0 {
            DistributionKind::Point { value } => DistributionMoments {
                mean: value,
                variance: 0.0,
            },
            DistributionKind::Normal {
                mean,
                standard_deviation,
            } => DistributionMoments {
                mean,
                variance: standard_deviation.powi(2),
            },
            DistributionKind::LogNormal { location, scale } => DistributionMoments {
                mean: (location + scale.powi(2) / 2.0).exp(),
                variance: scale.powi(2).exp_m1() * (2.0 * location + scale.powi(2)).exp(),
            },
            DistributionKind::Beta { alpha, beta } => beta_moments(alpha, beta, 0.0, 1.0),
            DistributionKind::ScaledBeta {
                alpha,
                beta,
                lower,
                upper,
            } => beta_moments(alpha, beta, lower, upper),
        }
    }

    /// Returns the exact population mean.
    pub fn mean(&self) -> f64 {
        self.moments().mean
    }

    /// Returns the exact population variance.
    pub fn variance(&self) -> f64 {
        self.moments().variance
    }

    /// Draws one deterministic sample from a pinned ChaCha20 stream.
    ///
    /// Repeating this call with the same distribution and seed is bit-reproducible
    /// for Optimist's pinned `rand`, `rand_chacha`, and `rand_distr` versions.
    /// Different library versions are not promised to preserve sample sequences.
    pub fn sample_seeded(&self, seed: u64) -> f64 {
        self.sample(&mut ChaCha20Rng::seed_from_u64(seed))
    }

    pub(super) fn sample(&self, rng: &mut ChaCha20Rng) -> f64 {
        match self.0 {
            DistributionKind::Point { value } => value,
            DistributionKind::Normal {
                mean,
                standard_deviation,
            } => Normal::new(mean, standard_deviation)
                .expect("validated normal")
                .sample(rng),
            DistributionKind::LogNormal { location, scale } => LogNormal::new(location, scale)
                .expect("validated log-normal")
                .sample(rng),
            DistributionKind::Beta { alpha, beta } => {
                Beta::new(alpha, beta).expect("validated beta").sample(rng)
            }
            DistributionKind::ScaledBeta {
                alpha,
                beta,
                lower,
                upper,
            } => {
                lower
                    + (upper - lower) * Beta::new(alpha, beta).expect("validated beta").sample(rng)
            }
        }
    }
}

fn beta_moments(alpha: f64, beta: f64, lower: f64, upper: f64) -> DistributionMoments {
    let total = alpha + beta;
    let width = upper - lower;
    DistributionMoments {
        mean: lower + width * alpha / total,
        variance: width.powi(2) * alpha * beta / (total.powi(2) * (total + 1.0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn exact_moments_cover_every_family() {
        let cases = [
            (Distribution::point(3.0).unwrap(), (3.0, 0.0)),
            (Distribution::normal(2.0, 3.0).unwrap(), (2.0, 9.0)),
            (
                Distribution::log_normal(0.0, 1.0).unwrap(),
                (0.5_f64.exp(), (1.0_f64.exp() - 1.0) * 1.0_f64.exp()),
            ),
            (Distribution::beta(2.0, 3.0).unwrap(), (0.4, 0.04)),
            (
                Distribution::scaled_beta(2.0, 3.0, -1.0, 4.0).unwrap(),
                (1.0, 1.0),
            ),
        ];
        for (distribution, (mean, variance)) in cases {
            assert!((distribution.mean() - mean).abs() < 1e-12);
            assert!((distribution.variance() - variance).abs() < 1e-12);
        }
    }

    #[test]
    fn seeded_samples_are_reproducible_and_respect_support() {
        let distributions = [
            Distribution::point(-2.0).unwrap(),
            Distribution::normal(1.0, 2.0).unwrap(),
            Distribution::log_normal(0.0, 0.5).unwrap(),
            Distribution::beta(0.5, 3.0).unwrap(),
            Distribution::scaled_beta(2.0, 5.0, -4.0, -1.0).unwrap(),
        ];
        for distribution in distributions {
            assert_eq!(
                distribution.sample_seeded(42).to_bits(),
                distribution.sample_seeded(42).to_bits()
            );
        }
        assert!(Distribution::log_normal(0.0, 0.5).unwrap().sample_seeded(7) > 0.0);
        assert!((0.0..=1.0).contains(&Distribution::beta(2.0, 5.0).unwrap().sample_seeded(7)));
        assert!(
            (-4.0..=-1.0).contains(
                &Distribution::scaled_beta(2.0, 5.0, -4.0, -1.0)
                    .unwrap()
                    .sample_seeded(7)
            )
        );
    }

    proptest! {
        #[test]
        fn beta_affine_moments_and_samples_obey_support(
            alpha in 0.1_f64..20.0,
            beta in 0.1_f64..20.0,
            lower in -100.0_f64..100.0,
            width in 0.01_f64..100.0,
            seed in any::<u64>(),
        ) {
            let base = Distribution::beta(alpha, beta).unwrap();
            let scaled = Distribution::scaled_beta(alpha, beta, lower, lower + width).unwrap();
            prop_assert!((scaled.mean() - (lower + width * base.mean())).abs() <= 1e-10 * width.max(1.0));
            prop_assert!((scaled.variance() - width.powi(2) * base.variance()).abs() <= 1e-10 * width.powi(2).max(1.0));
            let sample = scaled.sample_seeded(seed);
            prop_assert!((lower..=lower + width).contains(&sample));
        }
    }
}
