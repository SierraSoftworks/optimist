//! Resolving the behaviours attached to a relationship.

use std::collections::BTreeMap;

use crate::{
    squiggle::Value,
    system::{evaluate::EvaluationError, model::Relationship, mutator::Mutator},
};

use super::{
    Timing,
    parsing::{derive_seed, first_message, runtime, syntax},
    prepared::PreparedMutator,
};

pub(super) fn prepare_mutators(
    relationship: &Relationship,
    catalogue: &BTreeMap<String, Mutator>,
    globals: &BTreeMap<String, Value>,
    config: Timing,
) -> Result<Vec<PreparedMutator>, EvaluationError> {
    let mut prepared = Vec::new();
    for attached in &relationship.mutators {
        let owner = format!("{} to {}", relationship.from, relationship.to);
        let mutator = catalogue.get(attached.mutator.as_str()).ok_or_else(|| {
            EvaluationError::UnknownMutator {
                relationship: owner.clone(),
                mutator: attached.mutator.to_string(),
            }
        })?;
        let mut properties = BTreeMap::new();
        for (name, declaration) in &mutator.properties {
            let source = attached
                .properties
                .get(name)
                .or(declaration.default.as_ref())
                .ok_or_else(|| EvaluationError::MissingProperty {
                    component: format!("{} on relationship {owner}", mutator.id),
                    property: name.clone(),
                })?;
            let location = format!("property '{name}' of {} on {owner}", mutator.id);
            let program = syntax(source).map_err(|diagnostics| EvaluationError::Syntax {
                location: location.clone(),
                message: first_message(&diagnostics),
            })?;
            let seed = derive_seed(0, &format!("{owner}/{}", mutator.id), name);
            let value = runtime(seed, config.ensemble)?
                .evaluate_values(
                    &program,
                    globals
                        .iter()
                        .map(|(name, value)| (name.as_str(), value.clone())),
                )
                .map_err(|diagnostic| EvaluationError::Evaluation {
                    location,
                    message: diagnostic.message,
                })?;
            properties.insert(name.clone(), value);
        }
        for name in attached.properties.keys() {
            if !mutator.properties.contains_key(name) {
                return Err(EvaluationError::UnknownProperty {
                    component: format!("{} on relationship {owner}", mutator.id),
                    property: name.clone(),
                });
            }
        }
        let mut requests = Vec::new();
        for (signal, transform) in &mutator.requests {
            let program =
                syntax(&transform.expression).map_err(|diagnostics| EvaluationError::Syntax {
                    location: format!("request '{signal}' of {} on {owner}", mutator.id),
                    message: first_message(&diagnostics),
                })?;
            requests.push((signal.clone(), program));
        }
        let mut responses = Vec::new();
        for (signal, transform) in &mutator.responses {
            let program =
                syntax(&transform.expression).map_err(|diagnostics| EvaluationError::Syntax {
                    location: format!("response '{signal}' of {} on {owner}", mutator.id),
                    message: first_message(&diagnostics),
                })?;
            responses.push((signal.clone(), program));
        }
        prepared.push(PreparedMutator {
            id: mutator.id.to_string(),
            properties,
            requests,
            responses,
        });
    }
    Ok(prepared)
}
