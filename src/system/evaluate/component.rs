//! Evaluating one component against the current estimate of its inputs.

use std::collections::BTreeMap;

use crate::{
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
    let inbound = arrivals(
        plan,
        component,
        current,
        config,
        time,
        Direction::Request,
        links,
        runtime,
    )?;
    let outbound = arrivals(
        plan,
        component,
        current,
        config,
        time,
        Direction::Response,
        links,
        runtime,
    )?;
    let prior = previous.get(&component.id).cloned().unwrap_or_default();
    let mut scope = plan.globals.clone();
    scope.extend(component.properties.clone());
    scope.insert(TIME.to_owned(), Value::Number(time));
    scope.insert(STEP.to_owned(), Value::Number(config.step));
    scope.insert(
        STEADY.to_owned(),
        Value::Boolean(config.mode == SolveMode::Steady),
    );
    scope.insert(INBOUND.to_owned(), ported(inbound.clone()));
    scope.insert(OUTBOUND.to_owned(), ported(outbound.clone()));
    scope.insert(
        PREVIOUS.to_owned(),
        Value::Dictionary(zeroed(component, &prior.channels)),
    );

    let mut channels = BTreeMap::new();
    for (name, program) in &component.channels {
        let value = runtime
            .evaluate_values(
                program,
                scope
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.clone())),
            )
            .map_err(|diagnostic| EvaluationError::Evaluation {
                location: format!("channel '{name}' of component '{}'", component.id),
                message: diagnostic.message,
            })?;
        scope.insert(name.clone(), value.clone());
        channels.insert(name.clone(), value);
    }
    let responses = publish(&component.inbound, &component.id, &scope, runtime)?;
    let requests = publish(&component.outbound, &component.id, &scope, runtime)?;
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
    scope: &BTreeMap<String, Value>,
    runtime: &mut Runtime,
) -> Result<BTreeMap<String, BTreeMap<String, Value>>, EvaluationError> {
    let mut published = BTreeMap::new();
    for (name, port) in ports {
        let mut signals = BTreeMap::new();
        for (signal, _, program) in &port.publishes {
            let value = runtime
                .evaluate_values(
                    program,
                    scope
                        .iter()
                        .map(|(name, value)| (name.as_str(), value.clone())),
                )
                .map_err(|diagnostic| EvaluationError::Evaluation {
                    location: format!("signal '{signal}' of port '{name}' on '{component}'"),
                    message: diagnostic.message,
                })?;
            signals.insert(signal.clone(), value);
        }
        published.insert(name.clone(), signals);
    }
    Ok(published)
}

/// Wraps per-port flows so an expression can read `in.<port>.<signal>`.
fn ported(ports: BTreeMap<String, BTreeMap<String, Value>>) -> Value {
    Value::Dictionary(
        ports
            .into_iter()
            .map(|(name, signals)| (name, Value::Dictionary(signals)))
            .collect(),
    )
}

/// Fills in every channel the type declares so a first step reads zero.
fn zeroed(
    component: &PreparedComponent,
    channels: &BTreeMap<String, Value>,
) -> BTreeMap<String, Value> {
    component
        .component_type
        .channels
        .keys()
        .map(|name| {
            let value = channels.get(name).cloned().unwrap_or(Value::Number(0.0));
            (name.clone(), value)
        })
        .collect()
}
