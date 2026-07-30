//! Resolving one component against its type, its properties, and its wiring.

use std::collections::BTreeMap;

use crate::{
    squiggle::Value,
    system::{
        evaluate::EvaluationError,
        manifest::{ComponentType, PortArity},
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
    scaling::{Scaling, link_peers, link_scales},
};

/// Everything a component is resolved against, gathered so the wiring below
/// reads as one step rather than seven parameters repeated at each call.
pub(super) struct Context<'a> {
    pub(super) model: &'a SystemModel,
    pub(super) catalogue: &'a BTreeMap<String, ComponentType>,
    pub(super) mutators: &'a BTreeMap<String, Mutator>,
    pub(super) globals: &'a BTreeMap<String, Value>,
    pub(super) scaling: &'a BTreeMap<ComponentId, Scaling>,
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
        let (id, capacity, bandwidth) = link(
            relationship,
            &port,
            &peer_port,
            context.globals,
            context.config,
        )?;
        let (request_scale, response_scale, request_receive_scale, response_receive_scale) =
            link_scales(context.scaling, &relationship.from, &relationship.to);
        attach(
            &mut inbound,
            &component.id,
            &port,
            PreparedLink {
                peer: relationship.from.clone(),
                peer_port,
                id,
                capacity,
                bandwidth,
                request_scale,
                response_scale,
                request_receive_scale,
                response_receive_scale,
                peers: link_peers(context.scaling, &component.id, &relationship.from),
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
        let (id, capacity, bandwidth) = link(
            relationship,
            &peer_port,
            &port,
            context.globals,
            context.config,
        )?;
        let (request_scale, response_scale, request_receive_scale, response_receive_scale) =
            link_scales(context.scaling, &relationship.from, &relationship.to);
        attach(
            &mut outbound,
            &component.id,
            &port,
            PreparedLink {
                peer: relationship.to.clone(),
                peer_port,
                id,
                capacity,
                bandwidth,
                request_scale,
                response_scale,
                request_receive_scale,
                response_receive_scale,
                peers: link_peers(context.scaling, &component.id, &relationship.to),
                mutators: prepare_mutators(
                    relationship,
                    context.mutators,
                    context.globals,
                    context.config,
                )?,
            },
        )?;
    }
    let scaling = context
        .scaling
        .get(&component.id)
        .cloned()
        .unwrap_or_default();
    Ok(PreparedComponent {
        id: component.id.clone(),
        component_type,
        properties,
        channels,
        constraints,
        inbound,
        outbound,
        replicas: scaling.replicas,
    })
}

fn attach(
    ports: &mut BTreeMap<String, PreparedPort>,
    component: &ComponentId,
    port: &str,
    link: PreparedLink,
) -> Result<(), EvaluationError> {
    let attached = ports
        .get_mut(port)
        .ok_or_else(|| EvaluationError::UnknownPort {
            component: component.to_string(),
            port: port.to_owned(),
        })?;
    // A type declaring a single-relationship port is saying its channels read
    // one peer's figures rather than a reduction over several, and a reduction
    // is what a second relationship would silently hand them.
    if attached.arity == PortArity::One && !attached.links.is_empty() {
        return Err(EvaluationError::CrowdedPort {
            component: component.to_string(),
            port: port.to_owned(),
        });
    }
    attached.links.push(link);
    Ok(())
}
