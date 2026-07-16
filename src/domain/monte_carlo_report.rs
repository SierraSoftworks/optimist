use serde::Serialize;

use super::MonteCarloConfig;

/// Why deterministic Monte Carlo sampling stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConvergenceStatus {
    /// Every output met the configured mean-standard-error criterion.
    Converged,
    /// The attempt limit was reached before all outputs met the criterion.
    MaximumSamplesReached,
    /// Too few valid joint draws remained after invalid samples were discarded.
    InsufficientValidSamples,
}

/// Counts joint draws rejected by numerical failure category.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct InvalidSampleCounts {
    /// Draws where a generic formula ratio had an exactly zero denominator.
    pub zero_denominator: u64,
    /// Draws where a primitive sampler returned NaN or infinity.
    pub non_finite_primitive: u64,
    /// Draws where formula arithmetic overflowed or otherwise became non-finite.
    pub non_finite_result: u64,
}

impl InvalidSampleCounts {
    /// Returns the total rejected joint draws.
    pub const fn total(self) -> u64 {
        self.zero_denominator + self.non_finite_primitive + self.non_finite_result
    }
}

/// Reproducibility, convergence, and numerical stability metadata for one run.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MonteCarloDiagnostics {
    /// Seed used to initialize the pinned ChaCha20 random stream.
    pub seed: u64,
    /// Number of joint draws attempted, including rejected draws.
    pub attempted_samples: u64,
    /// Number of aligned finite joint draws included in estimates.
    pub valid_samples: u64,
    /// Rejected sample counts grouped by numerical cause.
    pub invalid_samples: InvalidSampleCounts,
    /// Validated stopping criterion used for this run.
    pub criterion: MonteCarloConfig,
    /// Whether and why the run stopped.
    pub status: ConvergenceStatus,
}

/// Monte Carlo estimates and their sampling uncertainty for one output.
///
/// Variance uses Bessel's correction. Mean standard error is $s/\sqrt n$.
/// Variance standard error uses the plug-in fourth-central-moment expression
/// $\sqrt{(\hat\mu_4-(n-3)s^4/(n-1))/n}$ and is unavailable below four valid
/// draws. These errors measure Monte Carlo noise, not model uncertainty, and do
/// not diagnose bias or tail non-convergence.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MonteCarloEstimate {
    /// Sample mean, absent when no joint draw was valid.
    pub mean: Option<f64>,
    /// Unbiased sample variance, absent below two valid draws.
    pub variance: Option<f64>,
    /// Estimated standard error of the sample mean.
    pub mean_standard_error: Option<f64>,
    /// Estimated standard error of the sample variance.
    pub variance_standard_error: Option<f64>,
}

/// Aligned estimates and covariance from a deterministic joint formula run.
///
/// The covariance matrix uses Bessel's correction and the root order supplied by
/// the caller. Any invalid root rejects the entire draw, preserving alignment.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct JointMonteCarloReport {
    /// Per-root estimates in caller-supplied order.
    pub estimates: Vec<MonteCarloEstimate>,
    /// Sample covariance matrix; entries are absent below two valid draws.
    pub covariance: Vec<Vec<Option<f64>>>,
    /// Reproducibility and stopping diagnostics.
    pub diagnostics: MonteCarloDiagnostics,
}
