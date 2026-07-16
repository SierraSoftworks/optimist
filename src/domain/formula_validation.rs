use std::collections::{BTreeMap, BTreeSet};

use super::{CompiledFormula, EstimateAddress, Formula, FormulaError, ProjectId, Unit};

pub(super) fn validate(
    project: &ProjectId,
    root: &Formula,
    formulas: &BTreeMap<EstimateAddress, Formula>,
) -> Result<CompiledFormula, FormulaError> {
    let mut context = Context {
        project,
        formulas,
        visiting: BTreeSet::new(),
        resolved: BTreeMap::new(),
        dependencies: Vec::new(),
    };
    let unit = context.formula(root)?;
    Ok(CompiledFormula {
        unit,
        dependencies: context.dependencies,
    })
}

struct Context<'a> {
    project: &'a ProjectId,
    formulas: &'a BTreeMap<EstimateAddress, Formula>,
    visiting: BTreeSet<EstimateAddress>,
    resolved: BTreeMap<EstimateAddress, Unit>,
    dependencies: Vec<EstimateAddress>,
}

impl Context<'_> {
    fn formula(&mut self, formula: &Formula) -> Result<Unit, FormulaError> {
        match formula {
            Formula::Literal { unit, .. } => Ok(unit.clone()),
            Formula::Reference { address } => self.reference(address),
            Formula::Sum { terms } => self.sum(terms),
            Formula::Product { factors } => self.product(factors),
            Formula::Ratio {
                numerator,
                denominator,
            } => self
                .formula(numerator)?
                .checked_divide(&self.formula(denominator)?)
                .map_err(FormulaError::from),
            Formula::Power { base, exponent } => self
                .formula(base)?
                .checked_power(*exponent)
                .map_err(FormulaError::from),
            Formula::Bounded {
                input,
                lower,
                upper,
            } => {
                if !lower.is_finite() || !upper.is_finite() || lower > upper {
                    return Err(FormulaError::InvalidBounds);
                }
                self.formula(input)
            }
        }
    }

    fn reference(&mut self, address: &EstimateAddress) -> Result<Unit, FormulaError> {
        if &address.project != self.project {
            return Err(FormulaError::CrossProjectReference {
                address: address.clone(),
                expected: self.project.clone(),
                actual: address.project.clone(),
            });
        }
        if let Some(unit) = self.resolved.get(address) {
            return Ok(unit.clone());
        }
        if !self.visiting.insert(address.clone()) {
            return Err(FormulaError::ReferenceCycle(address.clone()));
        }
        let formula = self
            .formulas
            .get(address)
            .ok_or_else(|| FormulaError::MissingReference(address.clone()))?;
        let unit = self.formula(formula)?;
        self.visiting.remove(address);
        self.resolved.insert(address.clone(), unit.clone());
        self.dependencies.push(address.clone());
        Ok(unit)
    }

    fn sum(&mut self, terms: &[Formula]) -> Result<Unit, FormulaError> {
        if terms.len() < 2 {
            return Err(FormulaError::TooFewOperands { operation: "sum" });
        }
        let expected = self.formula(&terms[0])?;
        for term in &terms[1..] {
            let actual = self.formula(term)?;
            if actual != expected {
                return Err(FormulaError::UnitMismatch { expected, actual });
            }
        }
        Ok(expected)
    }

    fn product(&mut self, factors: &[Formula]) -> Result<Unit, FormulaError> {
        if factors.len() < 2 {
            return Err(FormulaError::TooFewOperands {
                operation: "product",
            });
        }
        factors
            .iter()
            .try_fold(Unit::dimensionless(), |unit, factor| {
                unit.checked_multiply(&self.formula(factor)?)
                    .map_err(FormulaError::from)
            })
    }
}
