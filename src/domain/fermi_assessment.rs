use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    CompiledFormula, Distribution, Formula, FormulaSet, JointMonteCarloReport, MonteCarloConfig,
    MonteCarloError, ProjectId, Unit,
};

/// Support required by the primitive estimate receiving a Fermi recommendation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FermiEstimateSupport {
    /// Any finite real value, approximated by a Normal distribution.
    Real,
    /// Values at or above zero, approximated by a LogNormal distribution.
    NonNegative,
    /// Normalized state or probability on `[0, 1]`, approximated by Beta.
    Probability,
    /// Relationship influence on `[-1, 1]`, approximated by Scaled Beta.
    Signed,
}

/// Primitive recommendation derived from a sampled Fermi decomposition.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FermiRecommendation {
    /// The decomposition is deterministic and can be represented exactly.
    Exact {
        /// Point distribution preserving the sampled constant.
        distribution: Distribution,
        /// Central 90% interval of the recommended primitive.
        interval: FermiInterval,
    },
    /// A primitive family matched to the decomposition's sampled mean and variance.
    MomentMatched {
        /// Validated primitive approximation suitable for the requested support.
        distribution: Distribution,
        /// Central 90% interval of the recommended primitive.
        interval: FermiInterval,
        /// Limitation callers should retain with the elicitation provenance.
        warning: String,
    },
    /// The sampled moments cannot parameterize a primitive with the requested support.
    Unavailable {
        /// Actionable explanation of the incompatibility.
        reason: String,
    },
}

/// Central interval implied by a recommended primitive approximation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct FermiInterval {
    /// Probability mass between the reported bounds.
    pub probability: f64,
    /// Lower quantile of the interval.
    pub lower: f64,
    /// Upper quantile of the interval.
    pub upper: f64,
}

/// Validation, sampling, and approximation result for one proposed Fermi decomposition.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FermiAssessment {
    /// Unit and deterministic reference dependencies validated before sampling.
    pub compiled: CompiledFormula,
    /// Seeded Monte Carlo moments, errors, rejected draws, and convergence status.
    pub report: JointMonteCarloReport,
    /// Primitive approximation which the caller may explicitly apply to an estimate.
    pub recommendation: FermiRecommendation,
}

/// Failures which prevent a Fermi decomposition from being assessed.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum FermiAssessmentError {
    /// Formula validation or deterministic Monte Carlo sampling failed.
    #[error(transparent)]
    MonteCarlo(#[from] MonteCarloError),
    /// The decomposition's derived unit does not match the estimate being elicited.
    #[error("Fermi decomposition unit {actual:?} does not match target unit {expected:?}")]
    TargetUnitMismatch {
        /// Unit required by the target estimate slot.
        expected: Unit,
        /// Unit derived from the proposed decomposition.
        actual: Unit,
    },
}

/// Samples one unit-checked Fermi expression and recommends a primitive approximation.
///
/// Sampling uses the supplied deterministic Monte Carlo configuration and the empty
/// formula set, so this entry point accepts literal decompositions without stored
/// project references. The recommendation moment-matches the sampled mean and variance:
/// Normal uses $\mu=m,\sigma=\sqrt v$; LogNormal uses
/// $\sigma^2=\ln(1+v/m^2),\mu=\ln m-\sigma^2/2$; and a Beta on `[a,b]` uses
/// $p=(m-a)/(b-a)$ and $k=p(1-p)(b-a)^2/v-1$, with
/// $\alpha=pk,\beta=(1-p)k$. These are approximations, not claims that the composed
/// distribution belongs to the selected family; tail shape and multimodality are not
/// preserved. Monte Carlo uncertainty remains available in [`FermiAssessment::report`].
///
/// ```
/// use optimist::domain::{
///     assess_fermi, Distribution, FermiEstimateSupport, Formula, MonteCarloConfig,
///     ProjectId, Unit,
/// };
///
/// let root = Formula::Product {
///     factors: vec![
///         Formula::Literal {
///             distribution: Distribution::scaled_beta(3.0, 3.0, 0.6, 0.9)?,
///             unit: Unit::dimensionless(),
///         },
///         Formula::Literal {
///             distribution: Distribution::scaled_beta(3.0, 3.0, 0.7, 1.0)?,
///             unit: Unit::dimensionless(),
///         },
///     ],
/// };
/// let assessment = assess_fermi(
///     &ProjectId::new("delivery")?,
///     root,
///     FermiEstimateSupport::Probability,
///     Unit::dimensionless(),
///     MonteCarloConfig::new(42, 1_000, 10_000, 1e-3, 1e-2)?,
/// )?;
/// assert_eq!(assessment.report.estimates.len(), 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn assess_fermi(
    project: &ProjectId,
    formula: Formula,
    support: FermiEstimateSupport,
    expected_unit: Unit,
    config: MonteCarloConfig,
) -> Result<FermiAssessment, FermiAssessmentError> {
    let formulas = FormulaSet::default();
    let compiled = formulas
        .validate(project, &formula)
        .map_err(MonteCarloError::from)?;
    if compiled.unit != expected_unit {
        return Err(FermiAssessmentError::TargetUnitMismatch {
            expected: expected_unit,
            actual: compiled.unit,
        });
    }
    let report = formulas.sample_joint(project, &[formula], config)?;
    let recommendation = report.estimates.first().and_then(|estimate| {
        Some(recommend(
            support,
            estimate.mean?,
            estimate.variance?,
        ))
    }).unwrap_or_else(|| FermiRecommendation::Unavailable {
        reason: "The decomposition did not produce enough valid finite draws to estimate a distribution.".to_owned(),
    });
    Ok(FermiAssessment {
        compiled,
        report,
        recommendation,
    })
}

fn recommend(support: FermiEstimateSupport, mean: f64, variance: f64) -> FermiRecommendation {
    if variance == 0.0 {
        return if contains(support, mean) {
            FermiRecommendation::Exact {
                distribution: Distribution::point(mean).expect("sampled mean is finite"),
                interval: FermiInterval {
                    probability: 0.9,
                    lower: mean,
                    upper: mean,
                },
            }
        } else {
            unavailable(support, mean, variance)
        };
    }
    let distribution = match support {
        FermiEstimateSupport::Real => Distribution::normal(mean, variance.sqrt()),
        FermiEstimateSupport::NonNegative if mean > 0.0 => {
            let log_variance = (1.0 + variance / mean.powi(2)).ln();
            Distribution::log_normal(mean.ln() - log_variance / 2.0, log_variance.sqrt())
        }
        FermiEstimateSupport::Probability => fit_beta(mean, variance, 0.0, 1.0),
        FermiEstimateSupport::Signed => fit_beta(mean, variance, -1.0, 1.0),
        FermiEstimateSupport::NonNegative => return unavailable(support, mean, variance),
    };
    match distribution {
        Ok(distribution) => {
            let interval = FermiInterval {
                probability: 0.9,
                lower: distribution.inverse_cdf(0.05),
                upper: distribution.inverse_cdf(0.95),
            };
            FermiRecommendation::MomentMatched {
                distribution,
                interval,
                warning: "This primitive preserves sampled mean and variance, not the decomposition's tail shape or multimodality.".to_owned(),
            }
        }
        Err(_) => unavailable(support, mean, variance),
    }
}

fn fit_beta(
    mean: f64,
    variance: f64,
    lower: f64,
    upper: f64,
) -> Result<Distribution, super::DistributionError> {
    let width = upper - lower;
    let normalized_mean = (mean - lower) / width;
    let concentration = normalized_mean * (1.0 - normalized_mean) * width.powi(2) / variance - 1.0;
    let alpha = normalized_mean * concentration;
    let beta = (1.0 - normalized_mean) * concentration;
    if lower == 0.0 && upper == 1.0 {
        Distribution::beta(alpha, beta)
    } else {
        Distribution::scaled_beta(alpha, beta, lower, upper)
    }
}

fn contains(support: FermiEstimateSupport, value: f64) -> bool {
    match support {
        FermiEstimateSupport::Real => true,
        FermiEstimateSupport::NonNegative => value >= 0.0,
        FermiEstimateSupport::Probability => (0.0..=1.0).contains(&value),
        FermiEstimateSupport::Signed => (-1.0..=1.0).contains(&value),
    }
}

fn unavailable(support: FermiEstimateSupport, mean: f64, variance: f64) -> FermiRecommendation {
    FermiRecommendation::Unavailable {
        reason: format!(
            "Sampled mean {mean:.4} and variance {variance:.4} cannot parameterize a {support:?} primitive. Revise the decomposition or its bounds."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommends_exact_points_and_moment_matched_bounded_families() {
        assert!(matches!(
            recommend(FermiEstimateSupport::Probability, 0.4, 0.0),
            FermiRecommendation::Exact { .. }
        ));
        let FermiRecommendation::MomentMatched { distribution, .. } =
            recommend(FermiEstimateSupport::Probability, 0.6, 0.04)
        else {
            panic!("expected Beta recommendation")
        };
        assert!((distribution.mean() - 0.6).abs() < 1e-12);
        assert!((distribution.variance() - 0.04).abs() < 1e-12);

        let FermiRecommendation::MomentMatched { distribution, .. } =
            recommend(FermiEstimateSupport::Signed, -0.25, 0.1)
        else {
            panic!("expected Scaled Beta recommendation")
        };
        assert!((distribution.mean() + 0.25).abs() < 1e-12);
        assert!((distribution.variance() - 0.1).abs() < 1e-12);
    }

    #[test]
    fn reports_moments_incompatible_with_requested_support() {
        assert!(matches!(
            recommend(FermiEstimateSupport::Probability, 0.5, 0.5),
            FermiRecommendation::Unavailable { .. }
        ));
        assert!(matches!(
            recommend(FermiEstimateSupport::NonNegative, -1.0, 0.2),
            FermiRecommendation::Unavailable { .. }
        ));
    }
}
