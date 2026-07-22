use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{Formula, MonteCarloConfig};

const MAX_EQUATION_BYTES: usize = 4_096;
const MAX_VARIABLES: usize = 128;
const MAX_TEXT_BYTES: usize = 256;

/// Versioned expression language retained with a Fermi estimate source.
///
/// Optimist currently accepts a constrained Squiggle-compatible expression surface:
/// named variables, finite numeric literals, arithmetic, parentheses, and integer
/// powers. The persisted typed [`Formula`] remains the execution authority, while
/// this marker lets future migrations distinguish source-language semantics.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FermiExpressionLanguage {
    /// Optimist's first Squiggle-compatible expression subset.
    #[default]
    OptimistSquiggleV1,
}

/// One named uncertain variable retained with a user-authored Fermi equation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FermiVariable {
    /// Equation identifier using letters, digits, and underscores.
    pub name: String,
    /// Human-entered central estimate before distribution fitting.
    pub estimate: f64,
    /// Human-readable unit expression retained for review.
    pub unit: String,
    /// Elicitation rule used to construct this variable's literal distribution.
    pub uncertainty: FermiVariableUncertainty,
}

/// Retained uncertainty inputs for one Fermi variable.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FermiVariableUncertainty {
    /// Central 90% interval spans one tenth to ten times the estimate.
    OrderOfMagnitude,
    /// Custom three-point PERT elicitation around the central estimate.
    ThreePoint {
        /// Plausible lower endpoint.
        low: f64,
        /// Plausible upper endpoint.
        high: f64,
    },
}

/// Complete reviewable source used to derive one effective estimate distribution.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FermiEstimateDefinition {
    /// Source-language contract used to interpret [`Self::equation`].
    #[serde(default)]
    pub language: FermiExpressionLanguage,
    /// Human-authored infix equation.
    pub equation: String,
    /// Named elicited variables referenced by the equation.
    pub variables: Vec<FermiVariable>,
    /// Canonical typed formula generated from the equation and variables.
    pub formula: Formula,
    /// Deterministic sampling controls used to assess the formula.
    pub monte_carlo: MonteCarloConfig,
}

impl FermiEstimateDefinition {
    /// Validates bounded review metadata before the canonical formula is assessed.
    pub fn validated(self) -> Result<Self, FermiEstimateError> {
        let equation = self.equation.trim();
        if equation.is_empty() || equation.len() > MAX_EQUATION_BYTES {
            return Err(FermiEstimateError::InvalidEquation);
        }
        if self.variables.is_empty() || self.variables.len() > MAX_VARIABLES {
            return Err(FermiEstimateError::InvalidVariableCount);
        }
        let mut names = BTreeSet::new();
        for variable in &self.variables {
            validate_variable(variable)?;
            if !names.insert(variable.name.as_str()) {
                return Err(FermiEstimateError::DuplicateVariable(variable.name.clone()));
            }
        }
        Ok(self)
    }
}

/// Invalid retained Fermi authoring data or cached assessment state.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum FermiEstimateError {
    /// Equation text is empty or exceeds the bounded source size.
    #[error("a Fermi equation must contain 1 to 4,096 bytes")]
    InvalidEquation,
    /// Definitions require between one and 128 variables.
    #[error("a Fermi estimate requires between 1 and 128 variables")]
    InvalidVariableCount,
    /// A variable name is not a valid equation identifier.
    #[error("invalid Fermi variable name {0:?}")]
    InvalidVariableName(String),
    /// Variable names must be unique within one equation.
    #[error("duplicate Fermi variable name {0:?}")]
    DuplicateVariable(String),
    /// A retained unit expression is too large for review and transport.
    #[error("Fermi variable units must be at most 256 bytes")]
    UnitTooLarge,
    /// Central estimates and custom endpoints must be finite and ordered.
    #[error("Fermi variable {0:?} has invalid uncertainty values")]
    InvalidUncertainty(String),
    /// Order-of-magnitude uncertainty requires a positive central estimate.
    #[error("Fermi variable {0:?} must be positive for order-of-magnitude uncertainty")]
    NonPositiveOrderOfMagnitude(String),
    /// The cached effective distribution disagrees with the retained assessment.
    #[error("the cached Fermi result does not match its assessed recommendation")]
    ResultMismatch,
    /// An unavailable recommendation cannot be persisted as an estimate.
    #[error("a Fermi assessment without a recommendation cannot be persisted")]
    UnavailableRecommendation,
}

fn validate_variable(variable: &FermiVariable) -> Result<(), FermiEstimateError> {
    let mut characters = variable.name.chars();
    if variable.name.len() > MAX_TEXT_BYTES
        || !characters
            .next()
            .is_some_and(|value| value.is_ascii_alphabetic() || value == '_')
        || !characters.all(|value| value.is_ascii_alphanumeric() || value == '_')
    {
        return Err(FermiEstimateError::InvalidVariableName(
            variable.name.clone(),
        ));
    }
    if variable.unit.len() > MAX_TEXT_BYTES {
        return Err(FermiEstimateError::UnitTooLarge);
    }
    if !variable.estimate.is_finite() {
        return Err(FermiEstimateError::InvalidUncertainty(
            variable.name.clone(),
        ));
    }
    match variable.uncertainty {
        FermiVariableUncertainty::OrderOfMagnitude if variable.estimate <= 0.0 => Err(
            FermiEstimateError::NonPositiveOrderOfMagnitude(variable.name.clone()),
        ),
        FermiVariableUncertainty::ThreePoint { low, high }
            if !low.is_finite()
                || !high.is_finite()
                || low > variable.estimate
                || variable.estimate > high =>
        {
            Err(FermiEstimateError::InvalidUncertainty(
                variable.name.clone(),
            ))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Distribution, Unit};

    fn definition() -> FermiEstimateDefinition {
        FermiEstimateDefinition {
            language: FermiExpressionLanguage::OptimistSquiggleV1,
            equation: "people / households".to_owned(),
            variables: vec![FermiVariable {
                name: "people".to_owned(),
                estimate: 1_500_000.0,
                unit: "people".to_owned(),
                uncertainty: FermiVariableUncertainty::OrderOfMagnitude,
            }],
            formula: Formula::Literal {
                distribution: Distribution::point(1.0).unwrap(),
                unit: Unit::dimensionless(),
            },
            monte_carlo: MonteCarloConfig::new(42, 100, 1_000, 0.01, 0.01).unwrap(),
        }
    }

    #[test]
    fn validates_review_metadata_and_rejects_duplicate_variables() {
        assert!(definition().validated().is_ok());
        let mut duplicate = definition();
        duplicate.variables.push(duplicate.variables[0].clone());
        assert!(matches!(
            duplicate.validated(),
            Err(FermiEstimateError::DuplicateVariable(_))
        ));
    }

    #[test]
    fn defaults_legacy_definitions_to_the_first_source_contract() {
        let mut value = serde_json::to_value(definition()).unwrap();
        value.as_object_mut().unwrap().remove("language");

        let restored = serde_json::from_value::<FermiEstimateDefinition>(value).unwrap();
        assert_eq!(
            restored.language,
            FermiExpressionLanguage::OptimistSquiggleV1
        );
    }
}
