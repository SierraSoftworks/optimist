use serde::{Deserialize, Deserializer, de};
use thiserror::Error;

use super::online_moments::OnlineJointMoments;

const MAX_SAMPLES: u64 = 10_000_000;

/// Validated deterministic Monte Carlo controls.
///
/// Sampling stops after at least `minimum_samples` valid joint draws when every
/// output satisfies $SE(\bar X)\le a+r|\bar X|$, or after `maximum_samples`
/// attempted draws. A configuration is bit-reproducible with Optimist's pinned
/// ChaCha20 and distribution crate versions. See Robert and Casella, *Monte Carlo
/// Statistical Methods*, 2nd ed., chapters 2 and 4.
///
/// ```
/// use optimist::domain::MonteCarloConfig;
/// let config = MonteCarloConfig::new(42, 1_000, 100_000, 1e-3, 1e-2)?;
/// assert_eq!(config.seed(), 42);
/// # Ok::<(), optimist::domain::MonteCarloConfigError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct MonteCarloConfig {
    seed: u64,
    minimum_samples: u64,
    maximum_samples: u64,
    absolute_tolerance: f64,
    relative_tolerance: f64,
}

impl<'de> Deserialize<'de> for MonteCarloConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SerializedConfig {
            seed: u64,
            minimum_samples: u64,
            maximum_samples: u64,
            absolute_tolerance: f64,
            relative_tolerance: f64,
        }

        let value = SerializedConfig::deserialize(deserializer)?;
        Self::new(
            value.seed,
            value.minimum_samples,
            value.maximum_samples,
            value.absolute_tolerance,
            value.relative_tolerance,
        )
        .map_err(de::Error::custom)
    }
}

impl MonteCarloConfig {
    /// Creates bounded sampling controls with finite, non-negative tolerances.
    pub fn new(
        seed: u64,
        minimum_samples: u64,
        maximum_samples: u64,
        absolute_tolerance: f64,
        relative_tolerance: f64,
    ) -> Result<Self, MonteCarloConfigError> {
        if minimum_samples < 2 || maximum_samples < minimum_samples || maximum_samples > MAX_SAMPLES
        {
            return Err(MonteCarloConfigError::InvalidSampleBounds);
        }
        if !absolute_tolerance.is_finite()
            || !relative_tolerance.is_finite()
            || absolute_tolerance < 0.0
            || relative_tolerance < 0.0
            || (absolute_tolerance == 0.0 && relative_tolerance == 0.0)
        {
            return Err(MonteCarloConfigError::InvalidTolerance);
        }
        Ok(Self {
            seed,
            minimum_samples,
            maximum_samples,
            absolute_tolerance,
            relative_tolerance,
        })
    }

    /// Returns the deterministic RNG seed.
    pub const fn seed(self) -> u64 {
        self.seed
    }
    /// Returns the valid joint draws required before convergence.
    pub const fn minimum_samples(self) -> u64 {
        self.minimum_samples
    }
    /// Returns the maximum attempted joint draws.
    pub const fn maximum_samples(self) -> u64 {
        self.maximum_samples
    }
    /// Returns the absolute mean-standard-error tolerance.
    pub const fn absolute_tolerance(self) -> f64 {
        self.absolute_tolerance
    }
    /// Returns the relative mean-standard-error tolerance.
    pub const fn relative_tolerance(self) -> f64 {
        self.relative_tolerance
    }

    pub(super) fn converged(self, moments: &OnlineJointMoments, dimensions: usize) -> bool {
        moments.count() >= self.minimum_samples
            && (0..dimensions).all(|index| {
                let mean = moments.mean(index).unwrap_or_default();
                moments.mean_standard_error(index).is_some_and(|error| {
                    error <= self.absolute_tolerance + self.relative_tolerance * mean.abs()
                })
            })
    }
}

/// Validation failures for [`MonteCarloConfig`].
#[derive(Clone, Debug, Error, PartialEq)]
pub enum MonteCarloConfigError {
    /// Sample bounds require `2 <= minimum <= maximum <= 10,000,000`.
    #[error("sample bounds require 2 <= minimum <= maximum <= 10,000,000")]
    InvalidSampleBounds,
    /// Tolerances must be finite, non-negative, and not both zero.
    #[error("Monte Carlo tolerances must be finite, non-negative, and not both zero")]
    InvalidTolerance,
}
