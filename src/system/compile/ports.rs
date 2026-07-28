//! Resolving the ports a relationship attaches to, and what waits on the wire.

use std::collections::BTreeMap;

use crate::{
    squiggle::Value,
    system::{
        evaluate::{EvaluationError, LinkId},
        manifest::{ComponentType, Port, Ports},
        model::{ComponentId, Relationship, SystemModel},
    },
};

use super::{
    Timing,
    parsing::{compile, derive_seed, first_message, runtime, syntax},
    prepared::PreparedPort,
};

/// Parses each port's published expressions, ready to evaluate against channels.
pub(super) fn prepare_ports(
    component: &ComponentId,
    declared: &BTreeMap<String, Port>,
    side: &str,
) -> Result<BTreeMap<String, PreparedPort>, EvaluationError> {
    let mut prepared = BTreeMap::new();
    for (name, port) in declared {
        let mut publishes = Vec::new();
        for (signal, source) in &port.publishes {
            let location = format!("{side} port '{name}' signal '{signal}'");
            publishes.push((
                signal.clone(),
                source.trim().to_owned(),
                compile(component, &location, source)?,
            ));
        }
        prepared.insert(
            name.clone(),
            PreparedPort {
                links: Vec::new(),
                publishes,
            },
        );
    }
    Ok(prepared)
}

/// Resolves which ports a relationship attaches to at each end.
///
/// Returns the inbound port on the destination and the outbound port on the
/// source. A relationship that names neither is resolved against types that
/// declare exactly one port on the relevant side, so simple designs stay free of
/// wiring detail while an ambiguous one is refused rather than guessed at.
pub(super) fn endpoints(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    relationship: &Relationship,
) -> Result<(String, String), EvaluationError> {
    let resolve = |component: &ComponentId, named: Option<&String>, inbound: bool| {
        let declared = model
            .components
            .iter()
            .find(|candidate| &candidate.id == component)
            .and_then(|candidate| catalogue.get(candidate.component_type.as_str()))
            .map(|component_type| {
                if inbound {
                    &component_type.ports.inbound
                } else {
                    &component_type.ports.outbound
                }
            });
        let Some(declared) = declared else {
            return Err(EvaluationError::UnknownPort {
                component: component.to_string(),
                port: named.cloned().unwrap_or_else(|| "default".to_owned()),
            });
        };
        match named {
            Some(port) if declared.contains_key(port) => Ok(port.clone()),
            Some(port) => Err(EvaluationError::UnknownPort {
                component: component.to_string(),
                port: port.clone(),
            }),
            None => Ports::sole(declared)
                .cloned()
                .ok_or_else(|| EvaluationError::AmbiguousPort {
                    component: component.to_string(),
                    side: if inbound { "inbound" } else { "outbound" }.to_owned(),
                }),
        }
    };
    let to = resolve(&relationship.to, relationship.to_port.as_ref(), true)?;
    let from = resolve(&relationship.from, relationship.from_port.as_ref(), false)?;
    Ok((to, from))
}

/// Resolves a relationship's identity and how much it can hold.
///
/// Both ends of a relationship prepare it independently, so the identity has to
/// be derived from the endpoints rather than allocated, or the two ends would
/// disagree about whose backlog is whose.
pub(super) fn link(
    relationship: &Relationship,
    inbound_port: &str,
    outbound_port: &str,
    globals: &BTreeMap<String, Value>,
    config: Timing,
) -> Result<(LinkId, Value), EvaluationError> {
    let id = LinkId {
        from: relationship.from.clone(),
        from_port: outbound_port.to_owned(),
        to: relationship.to.clone(),
        to_port: inbound_port.to_owned(),
    };
    let source = relationship.capacity_source();
    let location = format!("capacity of relationship {id}");
    let program = syntax(source).map_err(|diagnostics| EvaluationError::Syntax {
        location: location.clone(),
        message: first_message(&diagnostics),
    })?;
    let seed = derive_seed(config.seed, &id.to_string(), "capacity");
    let capacity = runtime(seed, config.sample_count)?
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
    Ok((id, capacity))
}
