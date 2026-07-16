use std::collections::BTreeMap;

use rand_chacha::ChaCha20Rng;

use super::{EstimateAddress, Formula, FormulaSet, MonteCarloError, ProjectDependenceModel};

pub(super) fn validate(
    formulas: &FormulaSet,
    model: &ProjectDependenceModel,
) -> Result<(), MonteCarloError> {
    for address in model
        .residual_groups
        .iter()
        .flat_map(|group| &group.members)
    {
        match formulas.0.get(address) {
            None => return Err(MonteCarloError::MissingDependenceMember(address.clone())),
            Some(Formula::Literal { .. }) => {}
            Some(_) => {
                return Err(MonteCarloError::NonMarginalDependenceMember(
                    address.clone(),
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn populate(
    formulas: &FormulaSet,
    model: &ProjectDependenceModel,
    rng: &mut ChaCha20Rng,
    memo: &mut BTreeMap<EstimateAddress, f64>,
) {
    for group in &model.residual_groups {
        let draw = group.correlation.sample(rng);
        for (address, probability) in group.members.iter().zip(draw.uniforms) {
            let Formula::Literal { distribution, .. } = &formulas.0[address] else {
                unreachable!("validated dependence member")
            };
            memo.insert(address.clone(), distribution.inverse_cdf(probability));
        }
    }
}
