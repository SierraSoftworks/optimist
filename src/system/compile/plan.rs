//! Resolving a whole model into a plan the solver can execute.

use std::collections::BTreeMap;

use crate::system::{
    evaluate::EvaluationError,
    manifest::ComponentType,
    model::SystemModel,
    mutator::Mutator,
    signal::{Signal, builtin_signals},
};

use super::{
    Timing,
    component::{Context, prepare_component},
    prepared::Plan,
    properties::quantities,
    scaling::resolve_scaling,
};

/// Resolves a model into a plan the solver can execute at one point in time.
///
/// Shared quantities and properties may depend on the elapsed time, so a plan
/// describes one step rather than the whole horizon. Within a step every value
/// is fixed, which is what lets relaxation converge instead of chasing values
/// that moved underneath it.
///
/// Expressions are resolved through a cache, so a horizon pays to parse each of
/// them once rather than once per step. The values they produce are recomputed
/// every step, which is the part that can actually differ.
pub(crate) fn prepare(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    mutators: &BTreeMap<String, Mutator>,
    overrides: &BTreeMap<String, String>,
    config: Timing,
) -> Result<Plan, EvaluationError> {
    let globals = quantities(&model.scratchpad, overrides, config)?;
    let signals = vocabulary(catalogue, mutators);
    let scaling = resolve_scaling(model, &globals, config)?;
    let context = Context {
        model,
        catalogue,
        mutators,
        globals: &globals,
        scaling: &scaling,
        config,
    };
    let components = model
        .components
        .iter()
        .map(|component| prepare_component(&context, component))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Plan {
        components,
        globals,
        signals,
    })
}

/// Every signal name this model could carry, with how arrivals of it combine.
///
/// The shipped declarations are joined by any name a loaded type or behaviour
/// mentions, so a project that invents a signal gets it aggregated by the
/// default rather than silently dropped where flows are gathered.
fn vocabulary(
    catalogue: &BTreeMap<String, ComponentType>,
    mutators: &BTreeMap<String, Mutator>,
) -> BTreeMap<String, Signal> {
    let mut signals = builtin_signals().clone();
    for component_type in catalogue.values() {
        let ports = component_type
            .ports
            .inbound
            .values()
            .chain(component_type.ports.outbound.values());
        for port in ports {
            for name in port.publishes.keys() {
                signals.entry(name.clone()).or_default();
            }
        }
    }
    for mutator in mutators.values() {
        for name in mutator.requests.keys().chain(mutator.responses.keys()) {
            signals.entry(name.clone()).or_default();
        }
    }
    signals
}
