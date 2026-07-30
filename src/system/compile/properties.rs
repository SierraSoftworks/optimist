//! Evaluating the shared quantities and each component's properties.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    squiggle::Value,
    system::{
        evaluate::EvaluationError,
        manifest::ComponentType,
        model::{Component, ScratchpadEntry},
    },
};

use super::{
    Timing,
    parsing::{derive_seed, first_message, runtime, syntax},
};

/// Evaluates the shared quantities, with any rebound by an intervention
/// replaced before their dependants are evaluated.
///
/// Replacements are substituted in place rather than appended, so an entry that
/// refers to a rebound quantity sees the new value. That is what lets one
/// rebinding reach every part of a design that sized itself against it.
///
/// Takes the entries rather than the model so that a caller wanting the scope
/// one entry can see — a preview of what is being typed into it — can pass the
/// prefix ahead of it and get exactly what the solver would.
pub(crate) fn quantities(
    scratchpad: &[ScratchpadEntry],
    overrides: &BTreeMap<String, String>,
    config: Timing,
) -> Result<BTreeMap<String, Value>, EvaluationError> {
    let declared = scratchpad
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<BTreeSet<_>>();
    if let Some(name) = overrides
        .keys()
        .find(|name| !declared.contains(name.as_str()))
    {
        return Err(EvaluationError::UnknownQuantity {
            quantity: name.clone(),
        });
    }
    let mut globals = BTreeMap::new();
    for entry in scratchpad {
        let source = overrides.get(&entry.name).unwrap_or(&entry.expression);
        let program = syntax(source).map_err(|diagnostics| EvaluationError::Syntax {
            location: format!("scratchpad entry '{}'", entry.name),
            message: first_message(&diagnostics),
        })?;
        let value = runtime(
            derive_seed(config.seed, "scratchpad", &entry.name),
            config.ensemble,
        )?
        .evaluate_values(
            &program,
            globals
                .iter()
                .map(|(name, value): (&String, &Value)| (name.as_str(), value.clone()))
                .chain(config.clock()),
        )
        .map_err(|diagnostic| EvaluationError::Evaluation {
            location: format!("scratchpad entry '{}'", entry.name),
            message: diagnostic.message,
        })?;
        globals.insert(entry.name.clone(), value);
    }
    Ok(globals)
}

pub(super) fn evaluate_properties(
    component: &Component,
    component_type: &ComponentType,
    globals: &BTreeMap<String, Value>,
    config: Timing,
) -> Result<BTreeMap<String, Value>, EvaluationError> {
    let mut properties = BTreeMap::new();
    for (name, declaration) in &component_type.properties {
        let source = component
            .properties
            .get(name)
            .or(declaration.default.as_ref())
            .ok_or_else(|| EvaluationError::MissingProperty {
                component: component.id.to_string(),
                property: name.clone(),
            })?;
        let location = format!("property '{name}' of component '{}'", component.id);
        let program = syntax(source).map_err(|diagnostics| EvaluationError::Syntax {
            location: location.clone(),
            message: first_message(&diagnostics),
        })?;
        let seed = derive_seed(0, component.id.as_str(), name);
        let value = runtime(seed, config.ensemble)?
            .evaluate_values(
                &program,
                globals
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.clone()))
                    .chain(config.clock()),
            )
            .map_err(|diagnostic| EvaluationError::Evaluation {
                location,
                message: diagnostic.message,
            })?;
        properties.insert(name.clone(), value);
    }
    for name in component.properties.keys() {
        if !component_type.properties.contains_key(name) {
            return Err(EvaluationError::UnknownProperty {
                component: component.id.to_string(),
                property: name.clone(),
            });
        }
    }
    Ok(properties)
}
