use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::squiggle::{Diagnostic, Runtime, RuntimeConfig, Value, lint_program, parse};

use super::{Distribution, Unit};

const MAX_SOURCE_BYTES: usize = 65_536;
const MIN_SAMPLES: usize = 256;
const MAX_SAMPLES: usize = 4_096;

/// Complete support required by a Squiggle-authored estimate.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SquiggleEstimateSupport {
    /// Any finite real value.
    Real,
    /// Values at or above zero.
    NonNegative,
    /// Normalized state or probability on `[0, 1]`.
    Probability,
    /// Relationship influence on `[-1, 1]`.
    Signed,
    /// Native quantity constrained to an arbitrary inclusive interval.
    Bounded {
        /// Smallest legal value in the target quantity's native unit.
        lower: f64,
        /// Largest legal value in the target quantity's native unit.
        upper: f64,
    },
}

impl SquiggleEstimateSupport {
    pub(crate) fn accepts(self, distribution: &Distribution) -> bool {
        match self {
            Self::Real => true,
            Self::NonNegative => distribution.is_non_negative(),
            Self::Probability => distribution.is_probability(),
            Self::Signed => distribution.is_signed_influence(),
            Self::Bounded { lower, upper } => distribution.is_within(lower, upper),
        }
    }

    pub(crate) fn description(self) -> String {
        match self {
            Self::Real => "any finite real value".to_owned(),
            Self::NonNegative => "values zero or greater".to_owned(),
            Self::Probability => "values from 0 to 1".to_owned(),
            Self::Signed => "values from -1 to 1".to_owned(),
            Self::Bounded { lower, upper } => format!("values from {lower} to {upper}"),
        }
    }
}

/// Reviewable Squiggle source and deterministic evaluation controls for an estimate.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SquiggleEstimateDefinition {
    /// Squiggle calculation body whose final expression must be numeric or a distribution.
    pub source: String,
    /// Seed used to evaluate distribution algebra and retain effective draws.
    pub seed: u64,
    /// Number of effective draws retained for downstream model sampling.
    pub sample_count: usize,
    /// Canonical target unit enforced against the estimate owner.
    pub target_unit: Unit,
}

impl SquiggleEstimateDefinition {
    /// Validates bounded source and sampling controls.
    pub fn validated(self) -> Result<Self, SquiggleEstimateError> {
        if self.source.trim().is_empty() || self.source.len() > MAX_SOURCE_BYTES {
            return Err(SquiggleEstimateError::InvalidSourceSize);
        }
        if !(MIN_SAMPLES..=MAX_SAMPLES).contains(&self.sample_count) {
            return Err(SquiggleEstimateError::InvalidSampleCount);
        }
        Ok(self)
    }
}

/// Backend-generated summary retained with one Squiggle-authored estimate revision.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SquiggleEstimateAssessment {
    /// Runtime family produced by the authored calculation.
    pub family: String,
    /// Population or empirical mean when the family defines one.
    pub mean: Option<f64>,
    /// Population or empirical variance when the family defines one.
    pub variance: Option<f64>,
    /// Fifth percentile of the evaluated result.
    pub p05: f64,
    /// Median of the evaluated result.
    pub p50: f64,
    /// Ninety-fifth percentile of the evaluated result.
    pub p95: f64,
    /// Seed used for reproducible evaluation and runtime draws.
    pub seed: u64,
    /// Requested draw count used for predictive checks and empirical fallback.
    pub sample_count: usize,
}

/// Failures returned while validating or evaluating persisted Squiggle source.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum SquiggleEstimateError {
    /// Source must be nonempty and remain inside the transport/parser bound.
    #[error("Squiggle estimate source must contain 1 to 65,536 bytes")]
    InvalidSourceSize,
    /// Effective empirical approximations use a bounded sample count.
    #[error("Squiggle estimate sample count must be between 256 and 4,096")]
    InvalidSampleCount,
    /// Static or runtime diagnostics prevented evaluation.
    #[error("Squiggle estimate could not be evaluated: {0}")]
    Diagnostic(String),
    /// The calculation ended in a value that cannot represent a scalar estimate.
    #[error("Squiggle estimate must return a number or distribution, received {0}")]
    InvalidResult(String),
    /// A runtime distribution could not provide finite effective draws or quantiles.
    #[error("Squiggle estimate distribution is not sampleable: {0}")]
    InvalidDistribution(String),
    /// Client-supplied target unit disagrees with the estimate owner.
    #[error("Squiggle estimate target unit does not match its owner")]
    TargetUnitMismatch,
}

/// Evaluates source against a target unit and returns an effective domain distribution.
pub fn assess_squiggle_estimate(
    definition: SquiggleEstimateDefinition,
    expected_unit: &Unit,
) -> Result<
    (
        SquiggleEstimateDefinition,
        SquiggleEstimateAssessment,
        Distribution,
    ),
    SquiggleEstimateError,
> {
    let definition = definition.validated()?;
    if &definition.target_unit != expected_unit {
        return Err(SquiggleEstimateError::TargetUnitMismatch);
    }
    let source = wrapped_source(&definition.source, expected_unit);
    let program = parse(&source)
        .map_err(|diagnostics| SquiggleEstimateError::Diagnostic(diagnostics_text(&diagnostics)))?;
    if let Some(diagnostic) = lint_program(&program).into_iter().next() {
        return Err(SquiggleEstimateError::Diagnostic(diagnostic_text(
            &diagnostic,
        )));
    }
    let mut runtime = Runtime::with_config(RuntimeConfig {
        seed: definition.seed,
        sample_count: definition.sample_count,
        max_steps: 1_000_000,
    })
    .map_err(SquiggleEstimateError::InvalidDistribution)?;
    let value = runtime
        .evaluate_program(&program)
        .map_err(|diagnostic| SquiggleEstimateError::Diagnostic(diagnostic_text(&diagnostic)))?;
    let (family, mean, variance, p05, p50, p95, distribution, sample_count) = match value {
        Value::Number(value) if value.is_finite() => (
            "Number".to_owned(),
            Some(value),
            Some(0.0),
            value,
            value,
            value,
            Distribution::point(value).expect("finite point"),
            1,
        ),
        Value::Distribution(value) => {
            let effective = symbolic_distribution(&value).map_or_else(
                || {
                    let samples = (0..definition.sample_count)
                        .map(|index| value.sample_seeded(sample_seed(definition.seed, index)))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(SquiggleEstimateError::InvalidDistribution)?;
                    Distribution::empirical(samples).map_err(|error| {
                        SquiggleEstimateError::InvalidDistribution(error.to_string())
                    })
                },
                Ok,
            )?;
            (
                value.family().to_owned(),
                value.mean().ok().filter(|value| value.is_finite()),
                value.variance().ok().filter(|value| value.is_finite()),
                value
                    .quantile(0.05)
                    .map_err(SquiggleEstimateError::InvalidDistribution)?,
                value
                    .quantile(0.5)
                    .map_err(SquiggleEstimateError::InvalidDistribution)?,
                value
                    .quantile(0.95)
                    .map_err(SquiggleEstimateError::InvalidDistribution)?,
                effective,
                definition.sample_count,
            )
        }
        value => {
            return Err(SquiggleEstimateError::InvalidResult(
                value.type_name().to_owned(),
            ));
        }
    };
    Ok((
        definition.clone(),
        SquiggleEstimateAssessment {
            family,
            mean,
            variance,
            p05,
            p50,
            p95,
            seed: definition.seed,
            sample_count,
        },
        distribution,
    ))
}

fn symbolic_distribution(value: &crate::squiggle::Distribution) -> Option<Distribution> {
    if let Some(value) = value.point_value() {
        return Distribution::point(value).ok();
    }
    if let Some((mean, standard_deviation)) = value.normal_parameters() {
        return Distribution::normal(mean, standard_deviation).ok();
    }
    if let Some((location, scale)) = value.lognormal_parameters() {
        return Distribution::log_normal(location, scale).ok();
    }
    if let Some((alpha, beta)) = value.beta_parameters() {
        return Distribution::beta(alpha, beta).ok();
    }
    None
}

fn wrapped_source(source: &str, unit: &Unit) -> String {
    format!(
        "optimist_result :: {} = {{\n{}\n}}\noptimist_result",
        squiggle_unit(unit),
        source.trim()
    )
}

/// Renders a unit as a Squiggle `::` annotation.
///
/// Squiggle folds unit factors left-associatively, so a denominator is emitted
/// as one division per term: `a*b/c/d` groups as `((a*b)/c)/d`, which is
/// $ab/(cd)$. Joining the denominator with `*` instead would read as
/// $abd/c$, silently inverting every term after the first.
pub(super) fn squiggle_unit(unit: &Unit) -> String {
    let mut numerator = Vec::new();
    let mut denominator = Vec::new();
    for (name, exponent) in unit.terms() {
        let target = if exponent > 0 {
            &mut numerator
        } else {
            &mut denominator
        };
        let name = safe_unit_name(name);
        let magnitude = exponent.unsigned_abs();
        target.push(if magnitude == 1 {
            name
        } else {
            format!("{name}^{magnitude}")
        });
    }
    let numerator = if numerator.is_empty() {
        "1".to_owned()
    } else {
        numerator.join("*")
    };
    if denominator.is_empty() {
        numerator
    } else {
        format!("{numerator}/{}", denominator.join("/"))
    }
}

fn safe_unit_name(name: &str) -> String {
    if name
        .bytes()
        .enumerate()
        .all(|(index, byte)| byte.is_ascii_alphanumeric() || (index > 0 && byte == b'_'))
        && name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
    {
        return name.to_owned();
    }
    format!(
        "optimist_unit_{}",
        name.as_bytes()
            .iter()
            .map(|byte| format!("{byte:x}"))
            .collect::<Vec<_>>()
            .join("_")
    )
}

fn sample_seed(seed: u64, index: usize) -> u64 {
    seed.wrapping_add((index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15))
}

fn diagnostic_text(diagnostic: &Diagnostic) -> String {
    match &diagnostic.help {
        Some(help) => format!("{} ({help})", diagnostic.message),
        None => diagnostic.message.clone(),
    }
}

fn diagnostics_text(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(diagnostic_text)
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Squiggle folds unit factors left-associatively, so every denominator term
    /// needs its own division. Combining units in an expression is the only place
    /// this shows: a wrapper that renders and parses a unit the same wrong way
    /// still round-trips, which is how this survived until a ratio was taken.
    #[test]
    fn multi_term_denominators_survive_being_combined_in_an_expression() {
        let per_change_month =
            Unit::from_exponents([("minute", 1), ("change", -1), ("month", -1)]).unwrap();
        assert_eq!(squiggle_unit(&per_change_month), "minute/change/month");

        let definition = SquiggleEstimateDefinition {
            source: "cost :: minute/change/month = 3\nvolume :: change*month = 4\ncost * volume"
                .to_owned(),
            seed: 42,
            sample_count: 256,
            target_unit: Unit::base("minute").unwrap(),
        };
        let (_, assessment, _) =
            assess_squiggle_estimate(definition, &Unit::base("minute").unwrap()).unwrap();
        assert_eq!(assessment.mean, Some(12.0));
    }

    #[test]
    fn evaluates_rich_distributions_to_reproducible_empirical_results() {
        let definition = SquiggleEstimateDefinition {
            source: "gamma(4, 3) + triangular(0, 2, 5)".to_owned(),
            seed: 42,
            sample_count: 512,
            target_unit: Unit::base("day").unwrap(),
        };
        let first = assess_squiggle_estimate(definition.clone(), &Unit::base("day").unwrap());
        let second = assess_squiggle_estimate(definition, &Unit::base("day").unwrap());
        assert_eq!(first, second);
        let (_, assessment, effective) = first.unwrap();
        assert_eq!(assessment.sample_count, 512);
        assert!(assessment.p05 < assessment.p50 && assessment.p50 < assessment.p95);
        assert_eq!(
            serde_json::to_value(effective).unwrap()["type"],
            "empirical"
        );
    }

    #[test]
    fn preserves_supported_symbolic_distributions_without_effective_draws() {
        let definition = SquiggleEstimateDefinition {
            source: "Sym.beta(8, 2)".to_owned(),
            seed: 42,
            sample_count: 512,
            target_unit: Unit::dimensionless(),
        };
        let (_, assessment, effective) =
            assess_squiggle_estimate(definition, &Unit::dimensionless()).unwrap();
        assert_eq!(assessment.family, "Beta");
        assert!(effective.retained_draws().is_none());
        assert_eq!(serde_json::to_value(effective).unwrap()["type"], "beta");
    }

    #[test]
    fn persisted_workbench_sources_are_exactly_reproducible() {
        let cases = [
            ("beta(2, 2)", Unit::dimensionless()),
            ("beta(4, 2)", Unit::dimensionless()),
            ("beta(9, 1)", Unit::dimensionless()),
            (
                "lognormal(1.38629436112, 0.35)",
                Unit::base("duration").unwrap(),
            ),
            ("lognormal(2, 0.3)", Unit::base("duration").unwrap()),
            (
                "mixture([beta(8, 2), beta(3, 7)], [0.8, 0.2])",
                Unit::dimensionless(),
            ),
            ("normal(-2, 0.5)", Unit::base("day").unwrap()),
            ("pointMass(0.4)", Unit::dimensionless()),
            ("pointMass(0.5)", Unit::dimensionless()),
            ("pointMass(0.65)", Unit::dimensionless()),
            (
                "truncate(gamma(4, 3) + triangular(0, 2, 5), 0, 30)",
                Unit::base("day").unwrap(),
            ),
            (
                "result :: duration = gamma(4, 3)\nresult",
                Unit::base("duration").unwrap(),
            ),
            (
                "mechanism = beta(8, 2)\nevidence = beta(7, 3)\nmechanism * evidence * 2 - 1",
                Unit::dimensionless(),
            ),
        ];
        for (source, unit) in cases {
            let definition = SquiggleEstimateDefinition {
                source: source.to_owned(),
                seed: 42,
                sample_count: 2_048,
                target_unit: unit.clone(),
            };
            let first = assess_squiggle_estimate(definition.clone(), &unit).unwrap();
            let definition = serde_json::from_slice(
                &serde_json::to_vec(&definition).expect("definition serializes"),
            )
            .expect("definition round trips");
            let second = assess_squiggle_estimate(definition, &unit).unwrap();
            assert_eq!(first, second, "source was not reproducible: {source}");
        }
    }

    #[test]
    fn enforces_target_units_and_scalar_result_types() {
        let invalid_unit = SquiggleEstimateDefinition {
            source: "value :: hour = normal(4, 1)\nvalue".to_owned(),
            seed: 1,
            sample_count: 256,
            target_unit: Unit::base("day").unwrap(),
        };
        assert!(matches!(
            assess_squiggle_estimate(invalid_unit, &Unit::base("day").unwrap()),
            Err(SquiggleEstimateError::Diagnostic(_))
        ));
        let invalid_value = SquiggleEstimateDefinition {
            source: "[1, 2, 3]".to_owned(),
            seed: 1,
            sample_count: 256,
            target_unit: Unit::dimensionless(),
        };
        assert!(matches!(
            assess_squiggle_estimate(invalid_value, &Unit::dimensionless()),
            Err(SquiggleEstimateError::Diagnostic(_))
        ));
    }
}
