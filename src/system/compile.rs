//! Turning a model into a plan the solver can execute repeatedly.
//!
//! Everything that does not change between iterations is resolved once here:
//! expressions are parsed, scratchpad and property values are evaluated, and
//! each component's channels are put into an order that respects the references
//! between them. A solver may then run thousands of passes without reparsing a
//! single expression or rediscovering a single dependency.
//!
//! Property values are evaluated once for a second reason. Each is drawn against
//! a seed derived from the component and property that own it, so two components
//! declaring the same service time receive independent uncertainty while any one
//! property keeps the same draws on every pass. An iteration whose inputs were
//! redrawn each time would be chasing sampling noise rather than converging.

use std::collections::{BTreeMap, BTreeSet};

use crate::squiggle::{Runtime, RuntimeConfig, Value, ast::Program, parse};

use super::{
    evaluate::EvaluationError,
    expression::{STEP, TIME, free_names},
    manifest::ComponentType,
    model::{Component, ComponentId, Relationship, SystemModel},
    mutator::Mutator,
    scale_unit::{Distribution as ScaleDistribution, enclosing},
    signal::{Signal, builtin_signals},
};

/// When a plan is being resolved, and with what sampling budget.
#[derive(Clone, Copy, Debug)]
pub(super) struct Timing {
    pub(super) seed: u64,
    pub(super) sample_count: usize,
    pub(super) time: f64,
    pub(super) step: f64,
}

impl Timing {
    /// Returns the bindings describing when this plan is being resolved.
    fn clock(self) -> [(&'static str, Value); 2] {
        [
            (TIME, Value::Number(self.time)),
            (STEP, Value::Number(self.step)),
        ]
    }
}

/// One relationship arriving at a component, with its behaviours resolved.
pub(super) struct PreparedInbound {
    pub(super) source: ComponentId,
    pub(super) mutators: Vec<PreparedMutator>,
}

/// One behaviour attached to a relationship, ready to apply.
pub(super) struct PreparedMutator {
    pub(super) id: String,
    pub(super) properties: BTreeMap<String, Value>,
    pub(super) transforms: Vec<(String, Program)>,
}

/// One component resolved against its type and ready to evaluate.
pub(super) struct PreparedComponent {
    pub(super) id: ComponentId,
    pub(super) component_type: ComponentType,
    pub(super) properties: BTreeMap<String, Value>,
    pub(super) channels: Vec<(String, Program)>,
    pub(super) constraints: BTreeMap<String, (Program, Program)>,
    pub(super) upstream: Vec<PreparedInbound>,
    /// Replicas of this component across every enclosing scale unit.
    pub(super) replicas: f64,
    /// Share of the model's demand one replica serves.
    pub(super) share: f64,
}

/// A whole model resolved and ready to solve.
pub(super) struct Plan {
    pub(super) components: Vec<PreparedComponent>,
    pub(super) globals: BTreeMap<String, Value>,
    pub(super) signals: BTreeMap<String, Signal>,
}

/// Resolves a model into a plan the solver can execute at one point in time.
///
/// Shared quantities and properties may depend on the elapsed time, so a plan
/// describes one step rather than the whole horizon. Within a step every value
/// is fixed, which is what lets relaxation converge instead of chasing values
/// that moved underneath it.
///
/// Expressions are reparsed for each step. That is wasteful for a long horizon
/// and is the obvious thing to hoist once profiling says it matters; separating
/// structure from values before there is a reason to would have complicated the
/// plan for no measured gain.
pub(super) fn prepare(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    mutators: &BTreeMap<String, Mutator>,
    overrides: &BTreeMap<String, String>,
    config: Timing,
) -> Result<Plan, EvaluationError> {
    let globals = evaluate_scratchpad(model, overrides, config)?;
    let mut signals = builtin_signals();
    for component_type in catalogue.values() {
        for name in component_type.outputs.keys() {
            signals.entry(name.clone()).or_default();
        }
    }
    for mutator in mutators.values() {
        for name in mutator.transforms.keys() {
            signals.entry(name.clone()).or_default();
        }
    }
    let scaling = resolve_scaling(model, &globals, config)?;
    let mut components = Vec::new();
    for component in &model.components {
        let component_type = catalogue
            .get(component.component_type.as_str())
            .ok_or_else(|| EvaluationError::UnknownType {
                component: component.id.to_string(),
                component_type: component.component_type.to_string(),
            })?
            .clone();
        let properties = evaluate_properties(component, &component_type, &globals, config)?;
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
        let mut upstream = Vec::new();
        for relationship in model.inbound_to(&component.id) {
            upstream.push(PreparedInbound {
                source: relationship.from.clone(),
                mutators: prepare_mutators(relationship, mutators, &globals, config)?,
            });
        }
        let (replicas, share) = scaling.get(&component.id).copied().unwrap_or((1.0, 1.0));
        components.push(PreparedComponent {
            id: component.id.clone(),
            component_type,
            properties,
            channels,
            constraints,
            upstream,
            replicas,
            share,
        });
    }
    Ok(Plan {
        components,
        globals,
        signals,
    })
}

/// Resolves how many replicas of each component exist and what each one serves.
///
/// Replica counts multiply along the chain of enclosing units, while only the
/// sharded ones divide the load, so a component inside three mirrored regions of
/// ten shards is deployed thirty times and each copy serves a tenth of the
/// demand rather than a thirtieth.
fn resolve_scaling(
    model: &SystemModel,
    globals: &BTreeMap<String, Value>,
    config: Timing,
) -> Result<BTreeMap<ComponentId, (f64, f64)>, EvaluationError> {
    validate_scale_units(model)?;
    let mut counts = BTreeMap::new();
    for unit in &model.scale_units {
        let location = format!("replica count of scale unit '{}'", unit.id);
        let program = parse(&unit.replicas).map_err(|diagnostics| EvaluationError::Syntax {
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

fn prepare_mutators(
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
            let program = parse(source).map_err(|diagnostics| EvaluationError::Syntax {
                location: location.clone(),
                message: first_message(&diagnostics),
            })?;
            let seed = derive_seed(0, &format!("{owner}/{}", mutator.id), name);
            let value = runtime(seed, config.sample_count)?
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
        let mut transforms = Vec::new();
        for (signal, transform) in &mutator.transforms {
            let program =
                parse(&transform.expression).map_err(|diagnostics| EvaluationError::Syntax {
                    location: format!("transform '{signal}' of {} on {owner}", mutator.id),
                    message: first_message(&diagnostics),
                })?;
            transforms.push((signal.clone(), program));
        }
        prepared.push(PreparedMutator {
            id: mutator.id.to_string(),
            properties,
            transforms,
        });
    }
    Ok(prepared)
}

/// Evaluates the shared quantities, with any rebound by an intervention
/// replaced before their dependants are evaluated.
///
/// Replacements are substituted in place rather than appended, so an entry that
/// refers to a rebound quantity sees the new value. That is what lets one
/// rebinding reach every part of a design that sized itself against it.
fn evaluate_scratchpad(
    model: &SystemModel,
    overrides: &BTreeMap<String, String>,
    config: Timing,
) -> Result<BTreeMap<String, Value>, EvaluationError> {
    let declared = model
        .scratchpad
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
    for entry in &model.scratchpad {
        let source = overrides.get(&entry.name).unwrap_or(&entry.expression);
        let program = parse(source).map_err(|diagnostics| EvaluationError::Syntax {
            location: format!("scratchpad entry '{}'", entry.name),
            message: first_message(&diagnostics),
        })?;
        let value = runtime(
            derive_seed(config.seed, "scratchpad", &entry.name),
            config.sample_count,
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

fn evaluate_properties(
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
        let program = parse(source).map_err(|diagnostics| EvaluationError::Syntax {
            location: location.clone(),
            message: first_message(&diagnostics),
        })?;
        let seed = derive_seed(0, component.id.as_str(), name);
        let value = runtime(seed, config.sample_count)?
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

/// Sorts a component's channels so each is evaluated after what it refers to.
///
/// Channels within one component form a directed acyclic graph; a cycle among
/// them is an authoring error rather than feedback, because feedback travels
/// between components along relationships. Reporting it here names the channels
/// involved, where the solver could only report that nothing settled.
fn order_channels(
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
            parse(&channel.expression).map_err(|diagnostics| EvaluationError::Syntax {
                location: location.clone(),
                message: first_message(&diagnostics),
            })?;
        let references = free_names(&channel.expression)
            .map_err(|diagnostics| EvaluationError::Syntax {
                location,
                message: first_message(&diagnostics),
            })?
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

fn compile(component: &ComponentId, name: &str, source: &str) -> Result<Program, EvaluationError> {
    parse(source).map_err(|diagnostics| EvaluationError::Syntax {
        location: format!("constraint '{name}' of component '{component}'"),
        message: first_message(&diagnostics),
    })
}

pub(super) fn runtime(seed: u64, sample_count: usize) -> Result<Runtime, EvaluationError> {
    Runtime::with_config(RuntimeConfig {
        seed,
        sample_count,
        max_steps: 4_000_000,
    })
    .map_err(|message| EvaluationError::Evaluation {
        location: "runtime".to_owned(),
        message,
    })
}

/// Derives an independent stream for one named quantity.
///
/// Two components that declare the same service time are two separate estimates
/// and must vary independently, so the stream is keyed by owner and name rather
/// than shared. Mixing with an odd constant keeps neighbouring names from
/// producing neighbouring streams.
fn derive_seed(root: u64, owner: &str, name: &str) -> u64 {
    let mut hash = root ^ 0x9e37_79b9_7f4a_7c15;
    for byte in owner.bytes().chain([0]).chain(name.bytes()) {
        hash = hash.rotate_left(5) ^ u64::from(byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

fn first_message(diagnostics: &[crate::squiggle::Diagnostic]) -> String {
    diagnostics.first().map_or_else(
        || "invalid expression".to_owned(),
        |first| first.message.clone(),
    )
}
