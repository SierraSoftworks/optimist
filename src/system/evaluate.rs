//! Solving a system model for the quantities flowing through it.
//!
//! # Why iteration rather than ordering
//!
//! Component channels can be arranged in dependency order only when the model is
//! acyclic, and the models worth building rarely are. Utilisation sets queueing
//! delay, delay sets how long each request occupies a worker, occupancy sets
//! utilisation again. That loop has no first term to evaluate.
//!
//! The solver therefore relaxes toward a fixed point: it evaluates every
//! component against the current estimate of its inputs, blends the result part
//! of the way toward what it computed, and repeats until nothing moves. Where
//! the model happens to be acyclic this converges immediately, so ordering is
//! never needed as a special case.
//!
//! # Relaxation over sample sets
//!
//! Evaluation is elementwise across aligned draws, so each draw index carries a
//! deterministic system and settles on its own fixed point independently of the
//! others. Uncertainty is therefore not smeared through the loop: where demand
//! is uncertain enough that some draws saturate and others do not, the converged
//! result is a genuine mixture and its spread reports exactly how much of the
//! distribution has crossed into congestion.
//!
//! A single set of starting values is used, so where a loop admits more than one
//! fixed point the solver reports the one reachable from rest. That is the lower,
//! uncongested branch of a bistable system. The congested branch exists and is
//! not searched for; a wide converged distribution is the signal that the system
//! is operating near the fold between them.
//!
//! # Not converging is a result
//!
//! An iteration that never settles is reported rather than hidden behind a last
//! iterate. A loop whose gain exceeds one has no steady state to find, and
//! saying so is more useful than returning whichever values the cap happened to
//! stop at. The share of draws still moving is carried alongside the values so a
//! caller can tell a wholly unstable system from one unstable in its tail.

use std::collections::BTreeMap;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

use crate::squiggle::Value;

use super::{
    compile::{Plan, PreparedComponent, PreparedMutator, prepare, runtime},
    expression::{INBOUND, PREVIOUS, SIGNAL, STEP, TIME},
    manifest::ComponentType,
    model::{ComponentId, SystemModel},
    mutator::Mutator,
    values::{blend, distance, draws, from_draws},
};

/// How a model should be solved.
#[derive(Clone, Copy, Debug)]
pub struct EvaluationConfig {
    /// Root of the deterministic random stream.
    pub seed: u64,
    /// Draws carried through every quantity.
    pub sample_count: usize,
    /// Number of steps to advance.
    pub horizon: usize,
    /// Length of one step, in seconds.
    pub step: f64,
    /// Cap on relaxation passes within one step.
    pub max_iterations: usize,
    /// Largest relative movement treated as settled.
    pub tolerance: f64,
    /// Fraction of the way each pass moves toward its computed value.
    pub damping: f64,
}

impl Default for EvaluationConfig {
    fn default() -> Self {
        Self {
            seed: 0,
            sample_count: 1_000,
            horizon: 1,
            step: 1.0,
            max_iterations: 200,
            tolerance: 1e-6,
            damping: 0.5,
        }
    }
}

/// Why a model could not be solved.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvaluationError {
    /// A component adopts a type the catalogue does not define.
    UnknownType {
        /// The component.
        component: String,
        /// The type it named.
        component_type: String,
    },
    /// A relationship attaches a behaviour the catalogue does not define.
    UnknownMutator {
        /// The relationship, as source and destination.
        relationship: String,
        /// The behaviour it named.
        mutator: String,
    },
    /// A required property was not supplied and has no default.
    MissingProperty {
        /// The component.
        component: String,
        /// The property.
        property: String,
    },
    /// A supplied property is not declared by the component's type.
    UnknownProperty {
        /// The component.
        component: String,
        /// The property.
        property: String,
    },
    /// Channels within one component refer to each other in a cycle.
    ChannelCycle {
        /// The component.
        component: String,
        /// The channels that could not be ordered.
        channels: Vec<String>,
    },
    /// An expression could not be parsed.
    Syntax {
        /// Where the expression was declared.
        location: String,
        /// The first parser diagnostic.
        message: String,
    },
    /// An expression could not be evaluated.
    Evaluation {
        /// Where the expression was declared.
        location: String,
        /// The runtime diagnostic.
        message: String,
    },
}

impl std::fmt::Display for EvaluationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownType {
                component,
                component_type,
            } => write!(
                formatter,
                "component '{component}' adopts unknown type '{component_type}'"
            ),
            Self::UnknownMutator {
                relationship,
                mutator,
            } => write!(
                formatter,
                "relationship {relationship} attaches unknown behaviour '{mutator}'"
            ),
            Self::MissingProperty {
                component,
                property,
            } => write!(
                formatter,
                "component '{component}' does not supply required property '{property}'"
            ),
            Self::UnknownProperty {
                component,
                property,
            } => write!(
                formatter,
                "component '{component}' supplies '{property}', which its type does not declare"
            ),
            Self::ChannelCycle {
                component,
                channels,
            } => write!(
                formatter,
                "channels {channels:?} of component '{component}' refer to each other in a cycle"
            ),
            Self::Syntax { location, message } => {
                write!(formatter, "{location} does not parse: {message}")
            }
            Self::Evaluation { location, message } => {
                write!(formatter, "{location} failed to evaluate: {message}")
            }
        }
    }
}

impl std::error::Error for EvaluationError {}

/// The solved state of one component at one step.
#[derive(Clone, Debug, Default)]
pub struct ComponentState {
    /// Every channel the component's type declares.
    pub channels: BTreeMap<String, Value>,
    /// The signals published onto outbound relationships.
    pub outputs: BTreeMap<String, Value>,
}

/// The solved state of the whole model at one step.
#[derive(Clone, Debug)]
pub struct Step {
    /// Elapsed seconds at this step.
    pub time: f64,
    /// Per-component results.
    pub components: BTreeMap<ComponentId, ComponentState>,
    /// Whether relaxation settled within the iteration cap.
    pub converged: bool,
    /// Passes taken before settling or reaching the cap.
    pub iterations: usize,
    /// Largest relative movement in the final pass.
    pub movement: f64,
}

/// A solved model across its horizon.
#[derive(Clone, Debug)]
pub struct Evaluation {
    /// One entry per step, in time order.
    pub steps: Vec<Step>,
}

impl Evaluation {
    /// Borrows the final step, which is the steady state of a settled model.
    pub fn settled(&self) -> &Step {
        self.steps.last().expect("a horizon has at least one step")
    }

    /// Reports whether every step settled within the iteration cap.
    pub fn converged(&self) -> bool {
        self.steps.iter().all(|step| step.converged)
    }
}

/// Solves a model against a catalogue of component types.
pub fn evaluate(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    config: EvaluationConfig,
) -> Result<Evaluation, EvaluationError> {
    evaluate_with_mutators(model, catalogue, &builtin_mutators_or_empty(), config)
}

/// Solves a model against explicit component type and behaviour catalogues.
pub fn evaluate_with_mutators(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    mutators: &BTreeMap<String, Mutator>,
    config: EvaluationConfig,
) -> Result<Evaluation, EvaluationError> {
    let plan = prepare(model, catalogue, mutators, config.seed, config.sample_count)?;
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
    let mut previous: BTreeMap<ComponentId, ComponentState> = BTreeMap::new();
    let mut steps = Vec::with_capacity(config.horizon.max(1));
    for index in 0..config.horizon.max(1) {
        let time = index as f64 * config.step;
        let step = relax(&plan, &previous, time, config, &mut rng)?;
        previous.clone_from(&step.components);
        steps.push(step);
    }
    Ok(Evaluation { steps })
}

fn builtin_mutators_or_empty() -> BTreeMap<String, Mutator> {
    super::catalogue::builtin_mutators().unwrap_or_default()
}

fn relax(
    plan: &Plan,
    previous: &BTreeMap<ComponentId, ComponentState>,
    time: f64,
    config: EvaluationConfig,
    rng: &mut ChaCha20Rng,
) -> Result<Step, EvaluationError> {
    let mut current: BTreeMap<ComponentId, ComponentState> = plan
        .components
        .iter()
        .map(|component| {
            let state = previous.get(&component.id).cloned().unwrap_or_default();
            (component.id.clone(), state)
        })
        .collect();
    let mut movement = f64::INFINITY;
    let mut iterations = 0;
    while iterations < config.max_iterations {
        iterations += 1;
        movement = 0.0;
        for component in &plan.components {
            let computed = evaluate_component(plan, component, &current, previous, time, config)?;
            let settled = current
                .get(&component.id)
                .expect("every component was seeded above");
            let (blended, moved) = converge(component, settled, &computed, config, rng);
            movement = movement.max(moved);
            current.insert(component.id.clone(), blended);
        }
        if movement <= config.tolerance {
            break;
        }
    }
    Ok(Step {
        time,
        components: current,
        converged: movement <= config.tolerance,
        iterations,
        movement,
    })
}

fn evaluate_component(
    plan: &Plan,
    component: &PreparedComponent,
    current: &BTreeMap<ComponentId, ComponentState>,
    previous: &BTreeMap<ComponentId, ComponentState>,
    time: f64,
    config: EvaluationConfig,
) -> Result<ComponentState, EvaluationError> {
    let inbound = aggregate(plan, component, current, config, time)?;
    let prior = previous.get(&component.id).cloned().unwrap_or_default();
    let mut scope = plan.globals.clone();
    scope.extend(component.properties.clone());
    scope.insert(TIME.to_owned(), Value::Number(time));
    scope.insert(STEP.to_owned(), Value::Number(config.step));
    scope.insert(INBOUND.to_owned(), Value::Dictionary(inbound));
    scope.insert(
        PREVIOUS.to_owned(),
        Value::Dictionary(zeroed(component, &prior.channels)),
    );

    let mut runtime = runtime(config.seed, config.sample_count)?;
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
    let outputs = component
        .component_type
        .outputs
        .iter()
        .filter_map(|(signal, source)| {
            scope
                .get(source)
                .map(|value| (signal.clone(), value.clone()))
        })
        .collect();
    Ok(ComponentState { channels, outputs })
}

/// Sums each published signal across the relationships arriving at a component.
///
/// Each relationship's flow passes through its attached behaviours before being
/// counted, so a retry policy's amplification and a cache's absorption are
/// already reflected in what the component sees.
///
/// Every signal the catalogue knows about is present, defaulting to zero, so a
/// component at the edge of a model reads no demand rather than failing on a
/// missing key. Summation is right for rates and volumes, which is what flows
/// along a relationship today; a signal that composes some other way will need
/// its aggregation declared before a manifest can consume it.
fn aggregate(
    plan: &Plan,
    component: &PreparedComponent,
    current: &BTreeMap<ComponentId, ComponentState>,
    config: EvaluationConfig,
    time: f64,
) -> Result<BTreeMap<String, Value>, EvaluationError> {
    let mut totals = plan
        .signals
        .iter()
        .map(|signal| (signal.clone(), 0.0_f64))
        .collect::<BTreeMap<_, _>>();
    let mut uncertain: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for inbound in &component.upstream {
        let Some(state) = current.get(&inbound.source) else {
            continue;
        };
        let mut flow = plan
            .signals
            .iter()
            .map(|signal| (signal.clone(), Value::Number(0.0)))
            .collect::<BTreeMap<_, _>>();
        flow.extend(state.outputs.clone());
        for mutator in &inbound.mutators {
            flow = apply(plan, mutator, flow, config, time)?;
        }
        for (signal, value) in flow {
            match value {
                Value::Number(number) => *totals.entry(signal).or_default() += number,
                value => uncertain.entry(signal).or_default().push(value),
            }
        }
    }
    let mut inbound = totals
        .into_iter()
        .map(|(signal, total)| (signal, Value::Number(total)))
        .collect::<BTreeMap<_, _>>();
    for (signal, values) in uncertain {
        let certain = match inbound.get(&signal) {
            Some(Value::Number(number)) => *number,
            _ => 0.0,
        };
        inbound.insert(signal, sum(values, certain));
    }
    Ok(inbound)
}

/// Rewrites a flow through one attached behaviour.
///
/// Only the signals a behaviour declares are replaced; the rest travel on
/// untouched, so attaching a timeout does not silently discard the payload size
/// a downstream store needs.
fn apply(
    plan: &Plan,
    mutator: &PreparedMutator,
    flow: BTreeMap<String, Value>,
    config: EvaluationConfig,
    time: f64,
) -> Result<BTreeMap<String, Value>, EvaluationError> {
    let mut scope = plan.globals.clone();
    scope.extend(mutator.properties.clone());
    scope.insert(TIME.to_owned(), Value::Number(time));
    scope.insert(STEP.to_owned(), Value::Number(config.step));
    scope.insert(SIGNAL.to_owned(), Value::Dictionary(flow.clone()));

    let mut runtime = runtime(config.seed, config.sample_count)?;
    let mut rewritten = flow;
    for (signal, program) in &mutator.transforms {
        let value = runtime
            .evaluate_values(
                program,
                scope
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.clone())),
            )
            .map_err(|diagnostic| EvaluationError::Evaluation {
                location: format!("transform '{signal}' of behaviour '{}'", mutator.id),
                message: diagnostic.message,
            })?;
        rewritten.insert(signal.clone(), value);
    }
    Ok(rewritten)
}

fn sum(values: Vec<Value>, offset: f64) -> Value {
    let count = values
        .iter()
        .filter_map(|value| match value {
            Value::Distribution(distribution) => distribution.samples().map(<[f64]>::len),
            _ => None,
        })
        .min();
    let Some(count) = count else {
        return Value::Number(offset);
    };
    let mut rng = ChaCha20Rng::seed_from_u64(0);
    let mut total = vec![offset; count];
    for value in &values {
        let Some(draws) = draws(value, count, &mut rng) else {
            continue;
        };
        for (slot, draw) in total.iter_mut().zip(draws) {
            *slot += draw;
        }
    }
    from_draws(total).unwrap_or(Value::Number(offset))
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

fn converge(
    component: &PreparedComponent,
    settled: &ComponentState,
    computed: &ComponentState,
    config: EvaluationConfig,
    rng: &mut ChaCha20Rng,
) -> (ComponentState, f64) {
    let mut blended = ComponentState::default();
    let mut moved: f64 = 0.0;
    for (name, next) in &computed.channels {
        let Some(previous) = settled.channels.get(name) else {
            // Nothing to blend against on the first pass, so the computed value
            // stands and the step cannot yet be treated as settled.
            blended.channels.insert(name.clone(), next.clone());
            moved = f64::INFINITY;
            continue;
        };
        let count = config.sample_count;
        let (Some(previous), Some(next)) = (draws(previous, count, rng), draws(next, count, rng))
        else {
            blended.channels.insert(name.clone(), next.clone());
            continue;
        };
        moved = moved.max(distance(&previous, &next));
        let value =
            from_draws(blend(&previous, &next, config.damping)).unwrap_or(Value::Number(0.0));
        blended.channels.insert(name.clone(), value);
    }
    // An output publishes a named quantity, so it follows whichever blended
    // channel it names. A signal sourced from a property is constant and is
    // carried through as computed.
    for (signal, source) in &component.component_type.outputs {
        if let Some(value) = blended
            .channels
            .get(source)
            .or_else(|| computed.outputs.get(signal))
        {
            blended.outputs.insert(signal.clone(), value.clone());
        }
    }
    (blended, moved)
}
