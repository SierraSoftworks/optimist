use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{Distribution, EstimateAddress, ProjectId, Unit, UnitError, formula_validation};

/// A dimension-aware Fermi expression whose uncertain leaves are distributions or references.
///
/// This AST describes composition only; it does not sample or approximate a result.
/// `Bounded` means `min(max(input, lower), upper)` in the input's unit and will create
/// point mass at either bound when evaluated by a future sampling engine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Formula {
    /// Primitive uncertain value with an explicit runtime unit.
    Literal {
        /// Validated primitive distribution.
        distribution: Distribution,
        /// Physical or semantic unit carried by samples.
        unit: Unit,
    },
    /// Singular reference to another embedded estimate or Fermi component.
    Reference {
        /// Stable project-scoped address resolved by [`FormulaSet`].
        address: EstimateAddress,
    },
    /// Additive composition; every operand must have the same unit.
    Sum {
        /// Two or more terms whose sampled values will be added.
        terms: Vec<Formula>,
    },
    /// Multiplicative composition; operand units are multiplied.
    Product {
        /// Two or more factors whose sampled values will be multiplied.
        factors: Vec<Formula>,
    },
    /// Ratio whose unit is numerator divided by denominator.
    Ratio {
        /// Dividend expression.
        numerator: Box<Formula>,
        /// Divisor expression; numerical zero handling is deferred to evaluation.
        denominator: Box<Formula>,
    },
    /// Integer power which multiplies each base-unit exponent.
    Power {
        /// Expression being exponentiated.
        base: Box<Formula>,
        /// Integer exponent; negative powers invert dimensions.
        exponent: i32,
    },
    /// Clamps sampled values to finite bounds expressed in the input unit.
    Bounded {
        /// Expression whose samples will be clamped.
        input: Box<Formula>,
        /// Inclusive finite lower bound.
        lower: f64,
        /// Inclusive finite upper bound.
        upper: f64,
    },
}

/// Project-scoped formula definitions addressable by references.
///
/// A map entry is singular: repeated references to one address remain one dependency,
/// allowing a future sampler to draw it once per simulation rather than assuming
/// independent copies.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FormulaSet(BTreeMap<EstimateAddress, Formula>);

impl FormulaSet {
    /// Constructs a formula set, rejecting duplicate addresses.
    pub fn new(
        formulas: impl IntoIterator<Item = (EstimateAddress, Formula)>,
    ) -> Result<Self, FormulaError> {
        let mut values = BTreeMap::new();
        for (address, formula) in formulas {
            if values.insert(address.clone(), formula).is_some() {
                return Err(FormulaError::DuplicateAddress(address));
            }
        }
        Ok(Self(values))
    }

    /// Validates one root formula, deriving its unit and deterministic dependency order.
    pub fn validate(
        &self,
        project: &ProjectId,
        root: &Formula,
    ) -> Result<CompiledFormula, FormulaError> {
        formula_validation::validate(project, root, &self.0)
    }
}

/// Validated dimensional and dependency metadata for a formula.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompiledFormula {
    /// Unit derived from every expression and reference.
    pub unit: Unit,
    /// Unique referenced addresses in deterministic dependency-before-dependent order.
    pub dependencies: Vec<EstimateAddress>,
}

/// Structural or dimensional failures which make a Fermi expression invalid.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum FormulaError {
    /// The formula map contains the same estimate address more than once.
    #[error("duplicate formula address {0}")]
    DuplicateAddress(EstimateAddress),
    /// A referenced address is absent from the supplied formula set.
    #[error("missing formula reference {0}")]
    MissingReference(EstimateAddress),
    /// A formula references an estimate from another isolated project.
    #[error("formula reference {address} belongs to project {actual}, expected {expected}")]
    CrossProjectReference {
        /// Address which crossed the project boundary.
        address: EstimateAddress,
        /// Project being validated.
        expected: ProjectId,
        /// Project encoded by the address.
        actual: ProjectId,
    },
    /// References form a direct or indirect cycle.
    #[error("formula reference cycle includes {0}")]
    ReferenceCycle(EstimateAddress),
    /// Sum or product has fewer than two operands.
    #[error("{operation} requires at least two operands")]
    TooFewOperands {
        /// Operation whose arity is invalid.
        operation: &'static str,
    },
    /// Additive operands carry incompatible dimensions.
    #[error("sum unit mismatch: expected {expected:?}, found {actual:?}")]
    UnitMismatch {
        /// Unit established by the first term.
        expected: Unit,
        /// Unit found on a later term.
        actual: Unit,
    },
    /// Bounds are non-finite or ordered incorrectly.
    #[error("bounded transforms require finite lower <= upper")]
    InvalidBounds,
    /// Unit exponent arithmetic overflowed.
    #[error(transparent)]
    Unit(#[from] UnitError),
}
