//! Evaluating one component against the current estimate of its inputs.

use std::collections::BTreeMap;

use crate::{
    profile::time,
    squiggle::{Runtime, Value},
    system::{
        compile::{Plan, PreparedComponent, PreparedPort},
        expression::{INBOUND, OUTBOUND, PREVIOUS, STEADY, STEP, TIME},
        model::ComponentId,
    },
};

use super::{
    arrivals::arrivals,
    config::{EvaluationConfig, SolveMode},
    error::EvaluationError,
    flow::Direction,
    state::{ComponentState, LinkId, LinkState},
};

#[allow(clippy::too_many_arguments)]
pub(super) fn evaluate_component(
    plan: &Plan,
    component: &PreparedComponent,
    current: &BTreeMap<ComponentId, ComponentState>,
    previous: &BTreeMap<ComponentId, ComponentState>,
    time: f64,
    config: EvaluationConfig,
    links: &mut BTreeMap<LinkId, LinkState>,
    runtime: &mut Runtime,
) -> Result<ComponentState, EvaluationError> {
    let inbound = time!(
        Arrivals,
        arrivals(
            plan,
            component,
            current,
            config,
            time,
            Direction::Request,
            links,
            runtime,
        )
    )?;
    let outbound = time!(
        Arrivals,
        arrivals(
            plan,
            component,
            current,
            config,
            time,
            Direction::Response,
            links,
            runtime,
        )
    )?;
    // Bound once for the whole component. Every channel and every published
    // signal reads the same names, and two of them are the inbound and outbound
    // flows, so these are written straight into the runtime's scope: gathering
    // them into a map first and then copying that in built and tore down a
    // dictionary of every name, and copied the flows, twice per component per
    // pass.
    for (name, value) in &plan.globals {
        runtime.bind(name, value.clone());
    }
    for (name, value) in &component.properties {
        runtime.bind(name, value.clone());
    }
    runtime.bind(TIME, Value::Number(time));
    runtime.bind(STEP, Value::Number(config.step));
    runtime.bind(STEADY, Value::Boolean(config.mode == SolveMode::Steady));
    runtime.bind(INBOUND, ported(&inbound));
    runtime.bind(OUTBOUND, ported(&outbound));
    runtime.bind(
        PREVIOUS,
        Value::Dictionary(zeroed(component, previous.get(&component.id))),
    );

    let mut channels = BTreeMap::new();
    time!(Channels, {
        for (name, program) in &component.channels {
            let value = runtime.evaluate_bound(program).map_err(|diagnostic| {
                EvaluationError::Evaluation {
                    location: format!("channel '{name}' of component '{}'", component.id),
                    message: diagnostic.message,
                }
            })?;
            runtime.bind(name, value.clone());
            channels.insert(name.clone(), value);
        }
        Ok::<(), EvaluationError>(())
    })?;
    let responses = time!(Channels, publish(&component.inbound, &component.id, runtime))?;
    let requests = time!(Channels, publish(&component.outbound, &component.id, runtime))?;
    Ok(ComponentState {
        channels,
        requests,
        responses,
        arriving: inbound,
        returning: outbound,
    })
}

/// Evaluates each port's published expressions against the solved channels.
fn publish(
    ports: &BTreeMap<String, PreparedPort>,
    component: &ComponentId,
    runtime: &mut Runtime,
) -> Result<BTreeMap<String, BTreeMap<String, Value>>, EvaluationError> {
    let mut published = BTreeMap::new();
    for (name, port) in ports {
        let mut signals = BTreeMap::new();
        for (signal, _, program) in &port.publishes {
            let value = runtime.evaluate_bound(program).map_err(|diagnostic| {
                EvaluationError::Evaluation {
                    location: format!("signal '{signal}' of port '{name}' on '{component}'"),
                    message: diagnostic.message,
                }
            })?;
            signals.insert(signal.clone(), value);
        }
        published.insert(name.clone(), signals);
    }
    Ok(published)
}

/// Wraps per-port flows so an expression can read `in.<port>.<signal>`.
fn ported(ports: &BTreeMap<String, BTreeMap<String, Value>>) -> Value {
    Value::Dictionary(
        ports
            .iter()
            .map(|(name, signals)| (name.clone(), Value::Dictionary(signals.clone())))
            .collect(),
    )
}

/// Fills in every channel the type declares so a first step reads zero.
fn zeroed(
    component: &PreparedComponent,
    prior: Option<&ComponentState>,
) -> BTreeMap<String, Value> {
    component
        .component_type
        .channels
        .keys()
        .map(|name| {
            let value = prior
                .and_then(|prior| prior.channels.get(name))
                .cloned()
                .unwrap_or(Value::Number(0.0));
            (name.clone(), value)
        })
        .collect()
}
