//! Resolving how many replicas of each component exist and what each one serves.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    squiggle::Value,
    system::{
        evaluate::EvaluationError,
        model::{ComponentId, SystemModel},
        scale_unit::{Distribution as ScaleDistribution, enclosing},
    },
};

use super::{
    Timing,
    parsing::{derive_seed, first_message, runtime, syntax},
};

/// Resolves how many replicas of each component exist and what each one serves.
///
/// Replica counts multiply along the chain of enclosing units, while only the
/// sharded ones divide the load, so a component inside three mirrored regions of
/// ten shards is deployed thirty times and each copy serves a tenth of the
/// demand rather than a thirtieth.
pub(super) fn resolve_scaling(
    model: &SystemModel,
    globals: &BTreeMap<String, Value>,
    config: Timing,
) -> Result<BTreeMap<ComponentId, (f64, f64)>, EvaluationError> {
    validate_scale_units(model)?;
    let mut counts = BTreeMap::new();
    for unit in &model.scale_units {
        let location = format!("replica count of scale unit '{}'", unit.id);
        let program = syntax(&unit.replicas).map_err(|diagnostics| EvaluationError::Syntax {
            location: location.clone(),
            message: first_message(&diagnostics),
        })?;
        let value = runtime(
            derive_seed(0, "scale-unit", unit.id.as_str()),
            config.sample_count,
        )?
        .evaluate_values(
            &program,
            globals
                .iter()
                .map(|(name, value)| (name.as_str(), value.clone())),
        )
        .map_err(|diagnostic| EvaluationError::Evaluation {
            location: location.clone(),
            message: diagnostic.message,
        })?;
        let replicas = match value {
            Value::Number(number) => number,
            Value::Distribution(distribution) => distribution.mean().unwrap_or(1.0),
            _ => 1.0,
        };
        if !replicas.is_finite() || replicas < 1.0 {
            return Err(EvaluationError::Evaluation {
                location,
                message: "a scale unit must hold at least one replica".to_owned(),
            });
        }
        counts.insert(unit.id.clone(), (replicas, unit.distribution));
    }
    let mut scaling = BTreeMap::new();
    for (component, chain) in enclosing(&model.scale_units) {
        let mut replicas = 1.0;
        let mut share = 1.0;
        for unit in chain {
            let Some((count, distribution)) = counts.get(&unit) else {
                continue;
            };
            replicas *= count;
            if *distribution == ScaleDistribution::Sharded {
                share *= count;
            }
        }
        scaling.insert(component, (replicas, share));
    }
    Ok(scaling)
}

fn validate_scale_units(model: &SystemModel) -> Result<(), EvaluationError> {
    let known = model
        .scale_units
        .iter()
        .map(|unit| unit.id.clone())
        .collect::<BTreeSet<_>>();
    let mut claimed = BTreeSet::new();
    let components = model.identifiers();
    for unit in &model.scale_units {
        if let Some(parent) = &unit.parent
            && !known.contains(parent)
        {
            return Err(EvaluationError::UnknownScaleUnit {
                scale_unit: unit.id.to_string(),
                referenced: parent.to_string(),
            });
        }
        for member in &unit.members {
            if !components.contains(member) {
                return Err(EvaluationError::UnknownScaleUnit {
                    scale_unit: unit.id.to_string(),
                    referenced: member.to_string(),
                });
            }
            if !claimed.insert(member.clone()) {
                return Err(EvaluationError::SharedMembership {
                    component: member.to_string(),
                });
            }
        }
    }
    for unit in &model.scale_units {
        let mut seen = BTreeSet::from([unit.id.clone()]);
        let mut current = unit.parent.clone();
        while let Some(parent) = current {
            if !seen.insert(parent.clone()) {
                return Err(EvaluationError::ScaleUnitCycle {
                    scale_unit: unit.id.to_string(),
                });
            }
            current = model
                .scale_units
                .iter()
                .find(|candidate| candidate.id == parent)
                .and_then(|candidate| candidate.parent.clone());
        }
    }
    Ok(())
}
