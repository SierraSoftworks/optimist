use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{Distribution, DistributionError, quantile_fit};

/// Three elicited quantiles describing a team's prior belief about a quantity.
///
/// Probabilities are explicit because a lower/upper pair is meaningless without
/// its claimed coverage. Values are retained unchanged alongside any fitted family
/// so later calibration can compare the original elicitation with resolved outcomes.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct QuantileElicitation {
    /// Probability associated with the lower quantile, strictly between 0 and 0.5.
    pub lower_probability: f64,
    /// Elicited value at [`QuantileElicitation::lower_probability`].
    pub lower: f64,
    /// Elicited median at probability 0.5.
    pub median: f64,
    /// Probability associated with the upper quantile, strictly between 0.5 and 1.
    pub upper_probability: f64,
    /// Elicited value at [`QuantileElicitation::upper_probability`].
    pub upper: f64,
}

/// Residual diagnostics comparing a two-parameter family with three elicited points.
///
/// Non-zero residuals are expected when beliefs are asymmetric relative to the chosen
/// family. Callers should display these diagnostics rather than describing the fitted
/// distribution as an exact representation of the elicitation.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FitDiagnostics {
    /// Root mean squared error across lower, median, and upper values.
    pub root_mean_squared_error: f64,
    /// Largest absolute error among the three fitted quantiles.
    pub maximum_absolute_error: f64,
    /// Family-implied value at the lower elicited probability.
    pub fitted_lower: f64,
    /// Family-implied median.
    pub fitted_median: f64,
    /// Family-implied value at the upper elicited probability.
    pub fitted_upper: f64,
}

/// A validated primitive family together with its original elicitation and fit quality.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FittedDistribution {
    /// User-entered quantiles retained for audit and calibration.
    pub elicitation: QuantileElicitation,
    /// Validated Normal or LogNormal approximation used by analysis.
    pub distribution: Distribution,
    /// Quantitative mismatch between entered and family-implied quantiles.
    pub diagnostics: FitDiagnostics,
}

/// Failures which make a quantile elicitation or requested family invalid.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum QuantileFitError {
    /// At least one probability or value is NaN or infinite.
    #[error("quantile probabilities and values must be finite")]
    NonFinite,
    /// Lower and upper probabilities do not lie on opposite sides of 0.5.
    #[error("quantile probabilities must satisfy 0 < lower < 0.5 < upper < 1")]
    InvalidProbabilities,
    /// Quantile values are not monotonically non-decreasing.
    #[error("quantile values must satisfy lower <= median <= upper")]
    InvalidOrder,
    /// LogNormal fitting received a zero or negative elicited value.
    #[error("log-normal quantiles must be strictly positive")]
    NonPositiveLogNormalValue,
    /// Least-squares fitting produced a non-positive scale.
    #[error("elicited quantiles must imply a positive distribution scale")]
    InvalidScale,
    /// The fitted primitive distribution failed its own invariant checks.
    #[error(transparent)]
    Distribution(#[from] DistributionError),
}

impl QuantileElicitation {
    /// Fits a Normal prior using ordinary least squares in value space.
    ///
    /// For each elicited pair `(pᵢ, qᵢ)`, let `zᵢ = Φ⁻¹(pᵢ)`. The Normal quantile
    /// equation is `qᵢ = μ + σzᵢ`. Because three points overdetermine `μ` and `σ`,
    /// Optimist minimizes `Σ(qᵢ - μ - σzᵢ)²` and reports residual diagnostics.
    /// This assumes one unimodal unbounded Normal family; it does not prove the
    /// elicitor is calibrated. `Φ⁻¹` is provided by `statrs`' Normal CDF.
    pub fn fit_normal(self) -> Result<FittedDistribution, QuantileFitError> {
        quantile_fit::normal(self)
    }

    /// Fits a LogNormal prior using ordinary least squares in log space.
    ///
    /// For positive values, `ln(qᵢ) = μ + σΦ⁻¹(pᵢ)`. The same least-squares fit is
    /// applied to `ln(qᵢ)`, then diagnostics are evaluated back in the original
    /// quantity's units. This family assumes multiplicative positive uncertainty
    /// and is unsuitable when zero or negative values are plausible.
    pub fn fit_log_normal(self) -> Result<FittedDistribution, QuantileFitError> {
        quantile_fit::log_normal(self)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{QuantileElicitation, QuantileFitError};

    #[test]
    fn symmetric_normal_quantiles_fit_without_residual() {
        let fit = QuantileElicitation {
            lower_probability: 0.05,
            lower: 10.0,
            median: 20.0,
            upper_probability: 0.95,
            upper: 30.0,
        }
        .fit_normal()
        .unwrap();
        assert!(fit.diagnostics.maximum_absolute_error < 1e-12);
        assert!((fit.distribution.mean() - 20.0).abs() < 1e-12);
    }

    #[test]
    fn multiplicative_quantiles_fit_log_normal_in_log_space() {
        let fit = QuantileElicitation {
            lower_probability: 0.05,
            lower: 10.0,
            median: 20.0,
            upper_probability: 0.95,
            upper: 40.0,
        }
        .fit_log_normal()
        .unwrap();
        assert!(fit.diagnostics.maximum_absolute_error < 1e-10);
        assert!((fit.diagnostics.fitted_median - 20.0).abs() < 1e-10);
    }

    #[test]
    fn reports_family_mismatch_and_rejects_invalid_inputs() {
        let fit = QuantileElicitation {
            lower_probability: 0.1,
            lower: 1.0,
            median: 3.0,
            upper_probability: 0.9,
            upper: 10.0,
        }
        .fit_normal()
        .unwrap();
        assert!(fit.diagnostics.maximum_absolute_error > 1.0);
        assert_eq!(
            QuantileElicitation {
                lower_probability: 0.05,
                lower: 0.0,
                median: 1.0,
                upper_probability: 0.95,
                upper: 2.0,
            }
            .fit_log_normal(),
            Err(QuantileFitError::NonPositiveLogNormalValue)
        );
    }

    proptest! {
        #[test]
        fn normal_fit_is_affine_equivariant(
            center in -1.0e6_f64..1.0e6,
            half_width in 1.0e-3_f64..1.0e3,
        ) {
            let fit = QuantileElicitation {
                lower_probability: 0.05,
                lower: center - half_width,
                median: center,
                upper_probability: 0.95,
                upper: center + half_width,
            }
            .fit_normal()
            .unwrap();
            let tolerance = 1.0e-9 * (1.0 + center.abs() + half_width);
            prop_assert!(fit.diagnostics.maximum_absolute_error <= tolerance);
            prop_assert!((fit.distribution.mean() - center).abs() <= tolerance);
        }

        #[test]
        fn log_normal_fit_is_multiplicatively_equivariant(
            median in 1.0e-3_f64..1.0e3,
            factor in 1.01_f64..10.0,
        ) {
            let fit = QuantileElicitation {
                lower_probability: 0.05,
                lower: median / factor,
                median,
                upper_probability: 0.95,
                upper: median * factor,
            }
            .fit_log_normal()
            .unwrap();
            let tolerance = 1.0e-9 * (1.0 + median * factor);
            prop_assert!(fit.diagnostics.maximum_absolute_error <= tolerance);
            prop_assert!((fit.diagnostics.fitted_median - median).abs() <= tolerance);
        }
    }
}
