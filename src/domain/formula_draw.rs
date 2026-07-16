use std::collections::BTreeMap;

use rand_chacha::ChaCha20Rng;

use super::{EstimateAddress, Formula, FormulaSet};

#[derive(Clone, Copy)]
pub(super) enum SampleFailure {
    ZeroDenominator,
    NonFinitePrimitive,
    NonFiniteResult,
}

pub(super) fn draw(
    formulas: &FormulaSet,
    formula: &Formula,
    rng: &mut ChaCha20Rng,
    memo: &mut BTreeMap<EstimateAddress, f64>,
) -> Result<f64, SampleFailure> {
    let value = match formula {
        Formula::Literal { distribution, .. } => distribution.sample(rng),
        Formula::Reference { address } => {
            if let Some(value) = memo.get(address) {
                return Ok(*value);
            }
            let value = draw(
                formulas,
                formulas.0.get(address).expect("validated reference"),
                rng,
                memo,
            )?;
            memo.insert(address.clone(), value);
            return Ok(value);
        }
        Formula::Sum { terms } => terms.iter().try_fold(0.0, |total, term| {
            Ok(total + draw(formulas, term, rng, memo)?)
        })?,
        Formula::Product { factors } => factors.iter().try_fold(1.0, |total, factor| {
            Ok(total * draw(formulas, factor, rng, memo)?)
        })?,
        Formula::Ratio {
            numerator,
            denominator,
        } => {
            let numerator = draw(formulas, numerator, rng, memo)?;
            let denominator = draw(formulas, denominator, rng, memo)?;
            if denominator == 0.0 {
                return Err(SampleFailure::ZeroDenominator);
            }
            numerator / denominator
        }
        Formula::Power { base, exponent } => draw(formulas, base, rng, memo)?.powi(*exponent),
        Formula::Bounded {
            input,
            lower,
            upper,
        } => draw(formulas, input, rng, memo)?.clamp(*lower, *upper),
    };
    if value.is_finite() {
        Ok(value)
    } else if matches!(formula, Formula::Literal { .. }) {
        Err(SampleFailure::NonFinitePrimitive)
    } else {
        Err(SampleFailure::NonFiniteResult)
    }
}
