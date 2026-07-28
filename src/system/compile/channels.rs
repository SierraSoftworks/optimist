//! Ordering a component's channels so each is evaluated after what it refers to.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    squiggle::ast::Program,
    system::{
        evaluate::EvaluationError, expression::references, manifest::ComponentType,
        model::Component,
    },
};

use super::parsing::{first_message, syntax};

/// Sorts a component's channels so each is evaluated after what it refers to.
///
/// Channels within one component form a directed acyclic graph; a cycle among
/// them is an authoring error rather than feedback, because feedback travels
/// between components along relationships. Reporting it here names the channels
/// involved, where the solver could only report that nothing settled.
pub(super) fn order_channels(
    component: &Component,
    component_type: &ComponentType,
) -> Result<Vec<(String, Program)>, EvaluationError> {
    let names = component_type
        .channels
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut pending = BTreeMap::new();
    for (name, channel) in &component_type.channels {
        let location = format!("channel '{name}' of component '{}'", component.id);
        let program =
            syntax(&channel.expression).map_err(|diagnostics| EvaluationError::Syntax {
                location: location.clone(),
                message: first_message(&diagnostics),
            })?;
        let references = references(&program)
            .into_iter()
            .filter(|reference| names.contains(reference))
            .collect::<BTreeSet<_>>();
        pending.insert(name.clone(), (program, references));
    }

    let mut ordered = Vec::new();
    let mut placed = BTreeSet::new();
    while !pending.is_empty() {
        let ready = pending
            .iter()
            .find(|(_, (_, references))| references.iter().all(|name| placed.contains(name)))
            .map(|(name, _)| name.clone());
        let Some(name) = ready else {
            return Err(EvaluationError::ChannelCycle {
                component: component.id.to_string(),
                channels: pending.keys().cloned().collect(),
            });
        };
        let (program, _) = pending.remove(&name).expect("channel was found above");
        placed.insert(name.clone());
        ordered.push((name, program));
    }
    Ok(ordered)
}
