use crate::domain::{EstimateAddress, Formula};

use super::catalog::ProjectEntry;

pub(super) fn find(entry: &ProjectEntry, root: &EstimateAddress) -> Option<EstimateAddress> {
    entry
        .formulas
        .formulas
        .iter()
        .find_map(|(address, formula)| {
            if same_root(address, root) || references(formula, root) {
                Some(address.clone())
            } else {
                None
            }
        })
}

fn same_root(address: &EstimateAddress, root: &EstimateAddress) -> bool {
    address.project == root.project
        && address.owner == root.owner
        && address.estimate == root.estimate
}

fn references(formula: &Formula, target: &EstimateAddress) -> bool {
    match formula {
        Formula::Literal { .. } => false,
        Formula::Reference { address } => address == target,
        Formula::Sum { terms } => terms.iter().any(|term| references(term, target)),
        Formula::Product { factors } => factors.iter().any(|factor| references(factor, target)),
        Formula::Ratio {
            numerator,
            denominator,
        } => references(numerator, target) || references(denominator, target),
        Formula::Power { base, .. } => references(base, target),
        Formula::Bounded { input, .. } => references(input, target),
    }
}
