//! Resolving one component against its type, its properties, and its wiring.

use std::collections::BTreeMap;

use crate::{
    squiggle::Value,
    system::{
        evaluate::EvaluationError,
        manifest::ComponentType,
        model::{Component, ComponentId, SystemModel},
        mutator::Mutator,
    },
};

use super::{
    Timing,
    channels::order_channels,
    mutators::prepare_mutators,
    parsing::compile,
    ports::{endpoints, link, prepare_ports},
    prepared::{PreparedComponent, PreparedLink, PreparedPort},
    properties::evaluate_properties,
};

/// Everything a component is resolved against, gathered so the wiring below
/// reads as one step rather than seven parameters repeated at each call.
pub(super) struct Context<'a> {
    pub(super) model: &'a SystemModel,
    pub(super) catalogue: &'a BTreeMap<String, ComponentType>,
    pub(super) mutators: &'a BTreeMap<String, Mutator>,
    pub(super) globals: &'a BTreeMap<String, Value>,
    pub(super) scaling: &'a BTreeMap<ComponentId, (f64, f64)>,
    pub(super) config: Timing,
}

pub(super) fn prepare_component(
    context: &Context<'_>,
    component: &Component,
) -> Result<PreparedComponent, EvaluationError> {
    let component_type = context
        .catalogue
        .get(component.component_type.as_str())
        .ok_or_else(|| EvaluationError::UnknownType {
            component: component.id.to_string(),
            component_type: component.component_type.to_string(),
        })?
        .clone();
    let properties =
        evaluate_properties(component, &component_type, context.globals, context.config)?;
    let channels = order_channels(component, &component_type)?;
    let mut constraints = BTreeMap::new();
    for (name, constraint) in &component_type.constraints {
        constraints.insert(
            name.clone(),
            (
                compile(&component.id, name, &constraint.demand)?,
                compile(&component.id, name, &constraint.limit)?,
            ),
        );
    }
    let mut inbound = prepare_ports(&component.id, &component_type.ports.inbound, "inbound")?;
    let mut outbound = prepare_ports(&component.id, &component_type.ports.outbound, "outbound")?;
    for relationship in context.model.inbound_to(&component.id) {
        let (port, peer_port) = endpoints(context.model, context.catalogue, relationship)?;
        let (id, capacity) = link(
            relationship,
            &port,
            &peer_port,
            context.globals,
            context.config,
        )?;
        attach(
            &mut inbound,
            &component.id,
            &port,
            PreparedLink {
                peer: relationship.from.clone(),
                peer_port,
                id,
                capacity,
                mutators: prepare_mutators(
                    relationship,
                    context.mutators,
                    context.globals,
                    context.config,
                )?,
            },
        )?;
    }
    for relationship in context.model.outbound_from(&component.id) {
        let (peer_port, port) = endpoints(context.model, context.catalogue, relationship)?;
        let (id, capacity) = link(
            relationship,
            &peer_port,
            &port,
            context.globals,
            context.config,
        )?;
        attach(
            &mut outbound,
            &component.id,
            &port,
            PreparedLink {
                peer: relationship.to.clone(),
                peer_port,
                id,
                capacity,
                mutators: prepare_mutators(
                    relationship,
                    context.mutators,
                    context.globals,
                    context.config,
                )?,
            },
        )?;
    }
    let (replicas, share) = context
        .scaling
        .get(&component.id)
        .copied()
        .unwrap_or((1.0, 1.0));
    Ok(PreparedComponent {
        id: component.id.clone(),
        component_type,
        properties,
        channels,
        constraints,
        inbound,
        outbound,
        replicas,
        share,
    })
}

fn attach(
    ports: &mut BTreeMap<String, PreparedPort>,
    component: &ComponentId,
    port: &str,
    link: PreparedLink,
) -> Result<(), EvaluationError> {
    ports
        .get_mut(port)
        .ok_or_else(|| EvaluationError::UnknownPort {
            component: component.to_string(),
            port: port.to_owned(),
        })?
        .links
        .push(link);
    Ok(())
}
