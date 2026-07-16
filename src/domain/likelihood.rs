use serde::Serialize;
use thiserror::Error;

const MAX_EXACT_COUNT: u64 = 1_u64 << 53;

/// A validated Binomial likelihood summarized by successful and total trials.
///
/// Trials are exchangeable Bernoulli observations with a common, conditionally
/// independent success probability. This type does not model overdispersion,
/// changing rates, censoring, or correlated trials. See Bernardo and Smith,
/// *Bayesian Theory* (1994), section 5.2.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct BetaBinomialLikelihood {
    successes: u64,
    trials: u64,
}

impl BetaBinomialLikelihood {
    /// Creates a non-empty Binomial likelihood, rejecting impossible or imprecise counts.
    pub fn new(successes: u64, trials: u64) -> Result<Self, BayesianUpdateError> {
        if trials == 0 || successes > trials {
            return Err(BayesianUpdateError::InvalidBinomialCounts);
        }
        if trials > MAX_EXACT_COUNT {
            return Err(BayesianUpdateError::CountTooLarge);
        }
        Ok(Self { successes, trials })
    }

    /// Returns the number of successful trials.
    pub const fn successes(self) -> u64 {
        self.successes
    }

    /// Returns the total number of trials.
    pub const fn trials(self) -> u64 {
        self.trials
    }
}

/// A validated Normal likelihood for a sample mean with known observation variance.
///
/// `known_variance` is the variance of each conditionally independent observation,
/// not the variance of `sample_mean`; the latter is $\sigma^2/n$. This sufficient
/// statistic assumes exchangeable Normal observations and does not infer variance.
/// See Gelman et al., *Bayesian Data Analysis*, 3rd ed., section 2.5.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct NormalNormalLikelihood {
    sample_mean: f64,
    known_variance: f64,
    sample_count: u64,
}

impl NormalNormalLikelihood {
    /// Creates a Normal likelihood with finite mean, positive variance, and nonzero count.
    pub fn new(
        sample_mean: f64,
        known_variance: f64,
        sample_count: u64,
    ) -> Result<Self, BayesianUpdateError> {
        if !sample_mean.is_finite() || !known_variance.is_finite() {
            return Err(BayesianUpdateError::NonFiniteLikelihood);
        }
        if known_variance <= 0.0 {
            return Err(BayesianUpdateError::InvalidKnownVariance);
        }
        if sample_count == 0 {
            return Err(BayesianUpdateError::EmptySample);
        }
        if sample_count > MAX_EXACT_COUNT {
            return Err(BayesianUpdateError::CountTooLarge);
        }
        Ok(Self {
            sample_mean,
            known_variance,
            sample_count,
        })
    }

    /// Returns the observed sample mean.
    pub const fn sample_mean(self) -> f64 {
        self.sample_mean
    }
    /// Returns the known variance of each observation.
    pub const fn known_variance(self) -> f64 {
        self.known_variance
    }
    /// Returns the number of observations summarized by the sample mean.
    pub const fn sample_count(self) -> u64 {
        self.sample_count
    }
}

/// Failures from likelihood validation or conjugate posterior construction.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum BayesianUpdateError {
    /// A Binomial likelihood was empty or had more successes than trials.
    #[error("binomial likelihood requires 0 <= successes <= trials and trials > 0")]
    InvalidBinomialCounts,
    /// An integer count could not be represented exactly by the floating-point update.
    #[error("likelihood count exceeds the exact f64 integer range")]
    CountTooLarge,
    /// A likelihood parameter was NaN or infinite.
    #[error("likelihood parameters must be finite")]
    NonFiniteLikelihood,
    /// A Normal likelihood's known per-observation variance was not positive.
    #[error("known observation variance must be greater than zero")]
    InvalidKnownVariance,
    /// A Normal likelihood represented no observations.
    #[error("normal likelihood sample count must be greater than zero")]
    EmptySample,
    /// A Beta-Binomial update was requested for a non-Beta prior.
    #[error("beta-binomial updating requires a Beta prior")]
    ExpectedBetaPrior,
    /// A Normal-Normal update was requested for a non-Normal prior.
    #[error("normal-normal updating requires a Normal prior")]
    ExpectedNormalPrior,
    /// Floating-point arithmetic could not represent a finite posterior.
    #[error("posterior parameters are outside finite floating-point range")]
    NonFinitePosterior,
}
