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
    compile::{Plan, PreparedComponent, PreparedMutator, Timing, prepare, runtime},
    expression::{INBOUND, OUTBOUND, PREVIOUS, REQUEST, RESPONSE, SIGNAL, STEP, TIME},
    intervention::InterventionId,
    manifest::ComponentType,
    model::{ComponentId, SystemModel},
    mutator::Mutator,
    signal::Aggregation,
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
    /// Whether queues are solved for balance or advanced through time.
    pub mode: SolveMode,
}

impl Default for EvaluationConfig {
    fn default() -> Self {
        Self {
            seed: 0,
            sample_count: 1_000,
            horizon: 1,
            step: 1.0,
            // Feedback makes convergence slower than a feed-forward chain needs.
            // A retry policy against a saturated dependency has a loop gain just
            // under one, so the iterate approaches its fixed point steadily but
            // without hurry; stopping at a couple of hundred passes reports a
            // settled design as unsettled. A pass is cheap, and a loop that
            // genuinely has no fixed point diverges fast enough to be obvious
            // long before this cap.
            max_iterations: 1_500,
            tolerance: 1e-6,
            // Moving a fifth of the way rather than half. A cancelling timeout
            // and the load it relieves form an oscillator: cancelling lowers
            // utilisation, which lowers latency, which stops the cancelling,
            // which raises the load again. Half a step overshoots that on every
            // pass and the iterate cycles instead of settling. A fifth converges
            // on the same fixed point and takes more passes to get there, which
            // is the right trade when the alternative is reporting a design that
            // has a steady state as one that does not.
            damping: 0.2,
            mode: SolveMode::Steady,
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
    /// A relationship attaches to a port the component's type does not declare.
    UnknownPort {
        /// The component.
        component: String,
        /// The port it named.
        port: String,
    },
    /// A relationship names no port on a type that declares several.
    AmbiguousPort {
        /// The component.
        component: String,
        /// Which side of the component was ambiguous.
        side: String,
    },
    /// A scale unit refers to a component or unit the model does not declare.
    UnknownScaleUnit {
        /// The scale unit.
        scale_unit: String,
        /// The name it referred to.
        referenced: String,
    },
    /// A component is claimed directly by more than one scale unit.
    SharedMembership {
        /// The contested component.
        component: String,
    },
    /// Scale units enclose each other in a cycle.
    ScaleUnitCycle {
        /// A scale unit on the cycle.
        scale_unit: String,
    },
    /// An intervention rebinds a quantity the scratchpad does not declare.
    UnknownQuantity {
        /// The name it tried to rebind.
        quantity: String,
    },
    /// A model does not declare the requested intervention.
    UnknownIntervention {
        /// The identifier requested.
        intervention: String,
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
            Self::UnknownPort { component, port } => {
                write!(formatter, "component '{component}' has no port '{port}'")
            }
            Self::AmbiguousPort { component, side } => write!(
                formatter,
                "component '{component}' declares several {side} ports, so a relationship must name which one it uses"
            ),
            Self::UnknownScaleUnit {
                scale_unit,
                referenced,
            } => write!(
                formatter,
                "scale unit '{scale_unit}' refers to '{referenced}', which the model does not declare"
            ),
            Self::SharedMembership { component } => write!(
                formatter,
                "component '{component}' belongs to more than one scale unit; nest the units instead"
            ),
            Self::ScaleUnitCycle { scale_unit } => {
                write!(formatter, "scale unit '{scale_unit}' encloses itself")
            }
            Self::UnknownQuantity { quantity } => write!(
                formatter,
                "'{quantity}' is not a scratchpad quantity, so rebinding it would change nothing"
            ),
            Self::UnknownIntervention { intervention } => {
                write!(
                    formatter,
                    "the model declares no intervention '{intervention}'"
                )
            }
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
    /// Requests published on each outbound port, keyed by port then signal.
    pub requests: BTreeMap<String, BTreeMap<String, Value>>,
    /// Responses published on each inbound port, keyed by port then signal.
    pub responses: BTreeMap<String, BTreeMap<String, Value>>,
    /// Demand arriving on each inbound port, as the component read it.
    ///
    /// This is what `in.<port>.<signal>` resolved to, retained so a caller can
    /// report the load a component was under rather than only what it did about
    /// it.
    pub arriving: BTreeMap<String, BTreeMap<String, Value>>,
    /// Responses returning on each outbound port, as the component read them.
    ///
    /// This is what `out.<port>.<signal>` resolved to: the backpressure coming
    /// back from dependencies, which is what explains a component's own latency
    /// and failures.
    pub returning: BTreeMap<String, BTreeMap<String, Value>>,
}

/// How a model's queues are solved.
///
/// The same equations either way. What differs is whether the backlog on each
/// wire is asked to balance or asked to move.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SolveMode {
    /// Solve for the backlog that balances at the current load.
    ///
    /// One algebraic solve, using the closed form for a bounded queue, so the
    /// answer arrives immediately. This is what to use while a design is being
    /// edited, and what a constraint should be read against: it says where the
    /// design comes to rest, which is the question being asked nearly all of the
    /// time.
    ///
    /// It has no memory. Where a design has more than one resting state this
    /// reports the one reachable from nothing, so a surge that would have tipped
    /// it over and left it there appears to be survived.
    #[default]
    Steady,
    /// Advance the backlog through time, one step at a time.
    ///
    /// The queue on each wire fills and drains at a finite rate, which is what
    /// gives a design memory: a buffer filled by a surge has to be emptied
    /// afterwards, and if work arrives faster than it drains the design stays
    /// where the surge left it. Hysteresis, recovery time, and whether an
    /// incident ends when its cause does are only visible here.
    ///
    /// The cost is the step. Integration is only faithful while a step is short
    /// against the time a queue takes to drain, so a horizon that reads
    /// comfortably in seconds may need thousands of steps.
    Transient,
}

/// What is waiting on one relationship at one step.
///
/// A wire holds work that has been offered but not yet taken. Carrying that
/// backlog between steps is what gives a design inertia: load cannot appear at a
/// dependency the instant it is offered, and it cannot disappear the instant it
/// stops being offered either. A queue that filled during a surge has to drain
/// afterwards, and how long that takes is a property of the design rather than
/// of the solver.
#[derive(Clone, Debug)]
pub struct LinkState {
    /// Operations waiting on the wire.
    pub backlog: Value,
    /// Seconds an operation spends waiting, from Little's Law on the backlog.
    pub wait: Value,
    /// Share of offered operations refused because the wire was full.
    pub blocked: Value,
    /// Operations per second offered onto the wire, after the behaviours on it.
    pub offered: Value,
    /// Operations per second the far end can take.
    pub drain: Value,
}

impl Default for LinkState {
    /// An empty wire: nothing waiting, nothing delayed, nothing refused.
    fn default() -> Self {
        Self {
            backlog: Value::Number(0.0),
            wait: Value::Number(0.0),
            blocked: Value::Number(0.0),
            offered: Value::Number(0.0),
            drain: Value::Number(0.0),
        }
    }
}

/// Which relationship a piece of link state belongs to.
///
/// Derived from the endpoints rather than authored, because a relationship is
/// already identified by what it connects and asking authors to name their wires
/// as well would be a second thing to keep in step for no gain.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LinkId {
    /// Component the relationship leaves.
    pub from: ComponentId,
    /// Outbound port it leaves by.
    pub from_port: String,
    /// Component it arrives at.
    pub to: ComponentId,
    /// Inbound port it arrives at.
    pub to_port: String,
}

impl std::fmt::Display for LinkId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}.{} to {}.{}",
            self.from, self.from_port, self.to, self.to_port
        )
    }
}

/// The solved state of the whole model at one step.
#[derive(Clone, Debug)]
pub struct Step {
    /// Elapsed seconds at this step.
    pub time: f64,
    /// Per-component results.
    pub components: BTreeMap<ComponentId, ComponentState>,
    /// What is waiting on each relationship.
    pub links: BTreeMap<LinkId, LinkState>,
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
    evaluate_with_mutators(
        model,
        catalogue,
        &builtin_mutators_or_empty(),
        &BTreeMap::new(),
        config,
    )
}

/// Solves a model with one of its interventions applied.
///
/// The model is otherwise untouched, so any difference from the baseline is
/// attributable to the quantities the intervention rebinds.
pub fn evaluate_intervention(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    intervention: &InterventionId,
    config: EvaluationConfig,
) -> Result<Evaluation, EvaluationError> {
    let overrides = model.intervention(intervention)?.bindings();
    evaluate_with_mutators(
        model,
        catalogue,
        &builtin_mutators_or_empty(),
        &overrides,
        config,
    )
}

/// Solves a model against explicit catalogues and scratchpad replacements.
pub fn evaluate_with_mutators(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    mutators: &BTreeMap<String, Mutator>,
    overrides: &BTreeMap<String, String>,
    config: EvaluationConfig,
) -> Result<Evaluation, EvaluationError> {
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
    let mut previous: BTreeMap<ComponentId, ComponentState> = BTreeMap::new();
    let mut carried: BTreeMap<LinkId, LinkState> = BTreeMap::new();
    let mut steps = Vec::with_capacity(config.horizon.max(1));
    for index in 0..config.horizon.max(1) {
        let time = index as f64 * config.step;
        // Shared quantities may depend on the elapsed time, so the plan is
        // resolved afresh at each step and held fixed while it relaxes.
        let plan = prepare(
            model,
            catalogue,
            mutators,
            overrides,
            Timing {
                seed: config.seed,
                sample_count: config.sample_count,
                time,
                step: config.step,
            },
        )?;
        let step = relax(&plan, &previous, &carried, time, config, &mut rng)?;
        previous.clone_from(&step.components);
        carried.clone_from(&step.links);
        steps.push(step);
    }
    Ok(Evaluation { steps })
}

pub(super) fn builtin_mutators_or_empty() -> BTreeMap<String, Mutator> {
    super::catalogue::builtin_mutators().unwrap_or_default()
}

/// Smallest step the adaptive damping will tighten to.
const MINIMUM_DAMPING: f64 = 0.02;

/// Contracting passes that must pass before a tightened step is relaxed again.
const RECOVERY_PASSES: usize = 8;

fn relax(
    plan: &Plan,
    previous: &BTreeMap<ComponentId, ComponentState>,
    carried: &BTreeMap<LinkId, LinkState>,
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
    // Each wire starts the step where the last one left it. Solving for balance,
    // that is only a warm start and the passes below will move it; advancing
    // through time, it is the state itself, integrated once here and then held
    // still while everything else settles around it. Holding it still is the
    // point: a backlog that changed underneath the relaxation would be being
    // solved for rather than carried, which is the loop this mode exists to cut.
    let mut links: BTreeMap<LinkId, LinkState> = carried.clone();
    if config.mode == SolveMode::Transient {
        for component in &plan.components {
            for port in component
                .inbound
                .values()
                .chain(component.outbound.values())
            {
                for link in &port.links {
                    let before = links.get(&link.id).cloned().unwrap_or_default();
                    links.insert(link.id.clone(), advance(&before, &link.capacity, config));
                }
            }
        }
    }
    let mut movement = f64::INFINITY;
    let mut iterations = 0;
    // Damping starts where the caller asked and tightens itself when the
    // iterate overshoots. A model carries a thousand draws at once and they do
    // not all sit in the same regime: most settle readily while a few, drawn
    // near a fold, cycle at a step size the rest converge happily at. Halving
    // on any pass that moved further than the one before it lets those few
    // settle without slowing the rest more than they need, and means the figure
    // in the configuration is a starting point rather than something an author
    // has to get right.
    //
    // The tightening is undone again once the iterate has contracted steadily
    // for a while, because `movement` is the worst draw anywhere in the model:
    // a single draw crossing a fold early on would otherwise hold every other
    // draw at a twentieth of the step for the rest of the solve, which costs an
    // order of magnitude in passes to reach the same fixed point.
    let ceiling = config.damping.clamp(f64::EPSILON, 1.0);
    let mut weight = ceiling;
    let mut before = f64::INFINITY;
    let mut contracting = 0_usize;
    // One runtime serves the whole relaxation. Building the standard
    // environment costs more than evaluating the short expressions a model is
    // made of, and a solve evaluates them thousands of times.
    let mut runtime = runtime(config.seed, config.sample_count)?;
    while iterations < config.max_iterations {
        iterations += 1;
        movement = 0.0;
        for component in &plan.components {
            let computed = evaluate_component(
                plan,
                component,
                &current,
                previous,
                time,
                config,
                &mut links,
                &mut runtime,
            )?;
            let settled = current
                .get(&component.id)
                .expect("every component was seeded above");
            let (blended, moved) = converge(component, settled, &computed, weight, config, rng);
            movement = movement.max(moved);
            current.insert(component.id.clone(), blended);
        }
        if movement <= config.tolerance {
            break;
        }
        if movement > before * 1.05 {
            weight = (weight * 0.5).max(MINIMUM_DAMPING);
            contracting = 0;
        } else {
            contracting += 1;
            if contracting >= RECOVERY_PASSES {
                weight = (weight * 2.0).min(ceiling);
                contracting = 0;
            }
        }
        before = movement;
    }
    Ok(Step {
        time,
        components: current,
        links,
        converged: movement <= config.tolerance,
        iterations,
        movement,
    })
}

#[allow(clippy::too_many_arguments)]
fn evaluate_component(
    plan: &Plan,
    component: &PreparedComponent,
    current: &BTreeMap<ComponentId, ComponentState>,
    previous: &BTreeMap<ComponentId, ComponentState>,
    time: f64,
    config: EvaluationConfig,
    links: &mut BTreeMap<LinkId, LinkState>,
    runtime: &mut crate::squiggle::Runtime,
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
    ports: &BTreeMap<String, super::compile::PreparedPort>,
    component: &ComponentId,
    scope: &BTreeMap<String, Value>,
    runtime: &mut crate::squiggle::Runtime,
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

/// Which way along a relationship a set of flows is travelling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Direction {
    /// From caller to callee: the work being asked for.
    Request,
    /// From callee to caller: how serving it went.
    Response,
}

/// Signal names the wire itself acts on.
const RATE: &str = "rate";
const LATENCY: &str = "latency";
const SUCCESS: &str = "success";
const CAPACITY: &str = "capacity";

/// Solves the queue on one wire for the load crossing it.
///
/// A relationship holds work that has been offered but not yet taken. How much
/// waits, and how much is refused outright, follows from the ratio of what
/// arrives to what the far end can drain, against how deep the wire is. The
/// bounded results are used rather than the unbounded ones because a real buffer
/// fills and then refuses: reporting an ever-growing delay for a queue that
/// cannot grow would overstate latency and understate failure at exactly the
/// moment both matter.
///
/// This is the steady-state solution, so the backlog reported is the one that
/// balances at the current load rather than one integrated over time.
fn queued(
    request: &BTreeMap<String, Value>,
    response: &BTreeMap<String, Value>,
    capacity: &Value,
    config: EvaluationConfig,
) -> LinkState {
    let count = config.sample_count.max(1);
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
    let offered = request
        .get(RATE)
        .and_then(|value| draws(value, count, &mut rng));
    let drain = response
        .get(CAPACITY)
        .and_then(|value| draws(value, count, &mut rng));
    let depth = draws(capacity, count, &mut rng);
    let (Some(offered), Some(drain), Some(depth)) = (offered, drain, depth) else {
        return LinkState::default();
    };

    let mut backlog = Vec::with_capacity(count);
    let mut wait = Vec::with_capacity(count);
    let mut blocked = Vec::with_capacity(count);
    for index in 0..count {
        let rate = offered[index].max(0.0);
        let served = drain[index];
        let held = depth[index].max(0.0);
        // An unattached or unlimited dependency drains anything offered, so
        // nothing waits and nothing is refused.
        if !served.is_finite() || served <= 0.0 {
            backlog.push(0.0);
            wait.push(0.0);
            blocked.push(0.0);
            continue;
        }
        let utilisation = rate / served;
        let length = bounded_length(utilisation, held);
        backlog.push(length);
        wait.push(length / served);
        blocked.push(bounded_blocking(utilisation, held));
    }
    LinkState {
        backlog: from_draws(backlog).unwrap_or(Value::Number(0.0)),
        wait: from_draws(wait).unwrap_or(Value::Number(0.0)),
        blocked: from_draws(blocked).unwrap_or(Value::Number(0.0)),
        offered: request.get(RATE).cloned().unwrap_or(Value::Number(0.0)),
        drain: response
            .get(CAPACITY)
            .cloned()
            .unwrap_or(Value::Number(0.0)),
    }
}

/// Advances one wire's backlog by a step, from the flows it last carried.
///
/// Forward Euler on the contents of a bounded buffer. What arrived last step and
/// what left it are both known, so the difference is what accumulated, and the
/// buffer's depth bounds the result at both ends: it cannot hold less than
/// nothing, and once full the excess is refused rather than stored.
///
/// The rates are the previous step's on purpose. Nothing about this step is
/// consulted, which is what makes the pass explicit and breaks the loop that
/// otherwise ties a queue's delay to the demand that delay is producing. It is
/// also what makes the step size matter: advance further than the queue takes to
/// drain and the integration will overshoot and oscillate, in the solver rather
/// than in the design.
fn advance(before: &LinkState, capacity: &Value, config: EvaluationConfig) -> LinkState {
    let count = config.sample_count.max(1);
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
    let held = draws(&before.backlog, count, &mut rng);
    let offered = draws(&before.offered, count, &mut rng);
    let drain = draws(&before.drain, count, &mut rng);
    let depth = draws(capacity, count, &mut rng);
    let (Some(held), Some(offered), Some(drain), Some(depth)) = (held, offered, drain, depth)
    else {
        return before.clone();
    };

    let step = config.step.max(f64::EPSILON);
    let mut backlog = Vec::with_capacity(count);
    let mut wait = Vec::with_capacity(count);
    let mut blocked = Vec::with_capacity(count);
    for index in 0..count {
        let waiting = held[index].max(0.0);
        let rate = offered[index].max(0.0);
        let served = drain[index];
        let room = depth[index].max(0.0);
        if !served.is_finite() || served <= 0.0 {
            backlog.push(0.0);
            wait.push(0.0);
            blocked.push(0.0);
            continue;
        }
        // What the wire can take this step: whatever drains, plus whatever space
        // is left to store. Anything beyond that is turned away at the door.
        let admissible = served + (room - waiting).max(0.0) / step;
        let accepted = rate.min(admissible);
        let refused = if rate > 0.0 {
            ((rate - accepted) / rate).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let next = (waiting + (accepted - served) * step).clamp(0.0, room);
        backlog.push(next);
        wait.push(next / served);
        blocked.push(refused);
    }
    LinkState {
        backlog: from_draws(backlog).unwrap_or(Value::Number(0.0)),
        wait: from_draws(wait).unwrap_or(Value::Number(0.0)),
        blocked: from_draws(blocked).unwrap_or(Value::Number(0.0)),
        offered: before.offered.clone(),
        drain: before.drain.clone(),
    }
}

/// Mean number waiting in a buffer of `capacity` at this load.
///
/// The M/M/1/K result. Kept alongside the solver rather than reached through the
/// expression language because the wire is not something an author writes.
fn bounded_length(utilisation: f64, capacity: f64) -> f64 {
    if capacity <= 0.0 {
        return 0.0;
    }
    let rho = utilisation.max(0.0);
    if (rho - 1.0).abs() < 1e-9 {
        return capacity / 2.0;
    }
    let power = rho.powf(capacity + 1.0);
    if !power.is_finite() {
        return capacity;
    }
    let length = rho / (1.0 - rho) - (capacity + 1.0) * power / (1.0 - power);
    length.clamp(0.0, capacity)
}

/// Probability an arrival finds the buffer full and is refused.
fn bounded_blocking(utilisation: f64, capacity: f64) -> f64 {
    let rho = utilisation.max(0.0);
    if (rho - 1.0).abs() < 1e-9 {
        return 1.0 / (capacity + 1.0);
    }
    let power = rho.powf(capacity + 1.0);
    if !power.is_finite() {
        return (1.0 - 1.0 / rho).clamp(0.0, 1.0);
    }
    ((1.0 - rho) * rho.powf(capacity) / (1.0 - power)).clamp(0.0, 1.0)
}

/// Adds one quantity to another, draw by draw.
fn sum(left: &Value, right: &Value, config: EvaluationConfig) -> Value {
    elementwise(left, right, config, |a, b| a + b)
}

/// Reduces a success rate by the share that never got through.
fn survives(success: &Value, blocked: &Value, config: EvaluationConfig) -> Value {
    elementwise(success, blocked, config, |a, b| {
        a * (1.0 - b).clamp(0.0, 1.0)
    })
}

fn elementwise(
    left: &Value,
    right: &Value,
    config: EvaluationConfig,
    combine: impl Fn(f64, f64) -> f64,
) -> Value {
    let count = config.sample_count.max(1);
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
    let (Some(left), Some(right)) = (draws(left, count, &mut rng), draws(right, count, &mut rng))
    else {
        return Value::Number(0.0);
    };
    let combined = left
        .iter()
        .zip(&right)
        .map(|(a, b)| combine(*a, *b))
        .collect::<Vec<_>>();
    from_draws(combined).unwrap_or(Value::Number(0.0))
}

/// Collects the flows arriving on each of a component's ports, one direction.
///
/// Requests are gathered on inbound ports from the callers attached to them;
/// responses are gathered on outbound ports from the dependencies attached to
/// them. Both pass through the behaviours on the relationship before being
/// counted, so a retry policy's amplification and a timeout's cap are already
/// reflected in what the component reads.
///
/// Arrivals combine as their signal declares: rates add, latency takes the
/// largest, success multiplies, and per-operation quantities average. Extensive
/// signals are then divided by the component's share, so a component inside a
/// sharded scale unit reads the demand reaching one replica rather than the
/// whole fleet's. That is what makes a constraint answer "does one cell have
/// enough capacity", which is the question an engineer can act on.
///
/// Only signals that travel this way are present, each defaulting to its resting
/// value, so a component at the edge of a model reads no demand rather than
/// failing on a missing key, and can never read back the figures it publishes
/// itself.
#[allow(clippy::too_many_arguments)]
fn arrivals(
    plan: &Plan,
    component: &PreparedComponent,
    current: &BTreeMap<ComponentId, ComponentState>,
    config: EvaluationConfig,
    time: f64,
    direction: Direction,
    links: &mut BTreeMap<LinkId, LinkState>,
    runtime: &mut crate::squiggle::Runtime,
) -> Result<BTreeMap<String, BTreeMap<String, Value>>, EvaluationError> {
    let ports = match direction {
        Direction::Request => &component.inbound,
        Direction::Response => &component.outbound,
    };
    let own = current.get(&component.id);
    let mut gathered = BTreeMap::new();
    for (name, port) in ports {
        // Whatever this component publishes onto the port, before the wire has
        // had its say. The flows going the other way are what the mutators read,
        // and they read them as the caller would see them, so the queue below is
        // applied to this too.
        let mine = {
            let mut values = blank(plan);
            let published = own.and_then(|state| match direction {
                Direction::Request => state.responses.get(name),
                Direction::Response => state.requests.get(name),
            });
            values.extend(published.cloned().unwrap_or_default());
            values
        };
        let mut collected: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for link in &port.links {
            let published = current.get(&link.peer).and_then(|state| match direction {
                Direction::Request => state.requests.get(&link.peer_port),
                Direction::Response => state.responses.get(&link.peer_port),
            });
            let Some(published) = published else {
                continue;
            };
            let mut flow = blank(plan);
            flow.extend(published.clone());
            let mut counterpart = mine.clone();

            // The wire itself is a queue, and its cost lands on the caller: work
            // waits in front of the callee, and once the buffer is full the
            // excess is refused. Both show up in the response, as delay and as
            // failure.
            //
            // The response is rewritten whichever way this pass is gathering,
            // because both ends have to agree about it. Read from the caller it
            // is the answer arriving; read from the callee it is what the
            // behaviours on the wire are reacting to, and a retry policy that
            // saw the callee's unqueued success would never learn that the wire
            // in front of it was turning its requests away.
            //
            // The rate travelling the other way is left alone deliberately. It
            // is what was asked for, not what got through, and a component that
            // saw only what got through could never report being over its
            // capacity — the wire would have trimmed the demand to fit before it
            // arrived, and the one figure that says how badly a design is
            // undersized would always read exactly one.
            let (request, response) = match direction {
                Direction::Request => (&flow, &counterpart),
                Direction::Response => (&counterpart, &flow),
            };
            // What crosses the wire is what the behaviours on it produce, not
            // what the caller first offered. A retry policy reissuing a failed
            // call sends that call again, and the buffer in front of the callee
            // holds every one of those attempts. Measuring the queue against the
            // caller's original rate would let a retry storm be invisible to the
            // very queue it fills.
            //
            // Those behaviours are shown the wire as it stood on the last pass,
            // because what they do depends on how full it is and how full it is
            // depends on what they do. Relaxation is what resolves that: each
            // pass answers with the previous pass's queue and the two converge
            // together, which is the same treatment every other loop in the
            // model gets.
            let mut observed = response.clone();
            if let Some(before) = links.get(&link.id) {
                if let Some(latency) = observed.get_mut(LATENCY) {
                    *latency = sum(latency, &before.wait, config);
                }
                if let Some(success) = observed.get_mut(SUCCESS) {
                    *success = survives(success, &before.blocked, config);
                }
            }
            let mut crossing = request.clone();
            for mutator in &link.mutators {
                crossing = apply(
                    plan,
                    mutator,
                    crossing,
                    &observed,
                    Direction::Request,
                    config,
                    time,
                    runtime,
                )?;
            }
            let state = match config.mode {
                // Balance: the backlog is whatever the current load implies, so
                // it is recomputed as the load settles.
                SolveMode::Steady => queued(&crossing, response, &link.capacity, config),
                // Time: the backlog was fixed when the step began. Only the
                // flows are recorded, so the next step has something to advance
                // from once everything else has settled.
                SolveMode::Transient => {
                    let mut carried = links.get(&link.id).cloned().unwrap_or_default();
                    carried.offered = crossing.get(RATE).cloned().unwrap_or(Value::Number(0.0));
                    carried.drain = response
                        .get(CAPACITY)
                        .cloned()
                        .unwrap_or(Value::Number(0.0));
                    carried
                }
            };
            let queueing = match direction {
                Direction::Request => &mut counterpart,
                Direction::Response => &mut flow,
            };
            if let Some(latency) = queueing.get_mut(LATENCY) {
                *latency = sum(latency, &state.wait, config);
            }
            if let Some(success) = queueing.get_mut(SUCCESS) {
                *success = survives(success, &state.blocked, config);
            }
            links.insert(link.id.clone(), state);
            // Behaviours are declared in the order a request meets them, so a
            // response meets them in the opposite order. A timeout written
            // beneath a retry has to convert slowness into failure before the
            // retry above it decides whether there is anything to reissue;
            // applying them in declaration order both ways would let the retry
            // answer a question the timeout had not yet asked, and the design
            // would look as though its deadline cost nothing.
            let ordered: Vec<_> = match direction {
                Direction::Request => link.mutators.iter().collect(),
                Direction::Response => link.mutators.iter().rev().collect(),
            };
            for mutator in ordered {
                flow = apply(
                    plan,
                    mutator,
                    flow,
                    &counterpart,
                    direction,
                    config,
                    time,
                    runtime,
                )?;
            }
            for (signal, value) in flow {
                collected.entry(signal).or_default().push(value);
            }
        }

        let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
        let mut combined = BTreeMap::new();
        for (signal, declaration) in &plan.signals {
            let values = collected.remove(signal).unwrap_or_default();
            let divisor = if declaration.extensive {
                component.share
            } else {
                1.0
            };
            combined.insert(
                signal.clone(),
                combine(&values, declaration.aggregate, divisor, config, &mut rng),
            );
        }
        gathered.insert(name.clone(), combined);
    }
    Ok(gathered)
}

/// Every signal the catalogue knows about, at rest.
///
/// Success rests at one rather than zero. A component with nothing attached is
/// not failing, and starting the relaxation at zero would make every unattached
/// dependency look like a total outage.
fn blank(plan: &Plan) -> BTreeMap<String, Value> {
    plan.signals
        .iter()
        .map(|(signal, declaration)| {
            let rest = match declaration.aggregate {
                Aggregation::Product => 1.0,
                Aggregation::Min => f64::INFINITY,
                _ => 0.0,
            };
            (signal.clone(), Value::Number(rest))
        })
        .collect()
}

/// What a signal reads when nothing arrives carrying it.
///
/// The identity of the aggregation, so that combining nothing leaves the reader
/// unaffected. Zero is right for a rate, which nobody is offering, and wrong for
/// a success rate: a component with no dependencies depends on nothing that
/// could fail, and reading zero there would report every leaf of a design as a
/// total outage and propagate that back to the caller.
fn identity(aggregation: Aggregation) -> f64 {
    match aggregation {
        Aggregation::Product => 1.0,
        // Nothing attached imposes no ceiling, so the limit is unbounded rather
        // than nought; reading zero would report an unattached port as unable to
        // carry anything at all.
        Aggregation::Min => f64::INFINITY,
        Aggregation::Sum | Aggregation::Max | Aggregation::Mean => 0.0,
    }
}

/// Reduces several arrivals of one signal to the figure a component reads.
fn combine(
    values: &[Value],
    aggregation: Aggregation,
    divisor: f64,
    config: EvaluationConfig,
    rng: &mut ChaCha20Rng,
) -> Value {
    if values.is_empty() {
        return Value::Number(identity(aggregation));
    }
    let count = values
        .iter()
        .filter_map(|value| match value {
            Value::Distribution(distribution) => distribution.samples().map(<[f64]>::len),
            _ => None,
        })
        .min()
        .unwrap_or(config.sample_count.max(1));
    let columns = values
        .iter()
        .filter_map(|value| draws(value, count, rng))
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Value::Number(identity(aggregation));
    }
    let scale = if divisor > 0.0 { divisor } else { 1.0 };
    let combined = (0..count)
        .map(|index| {
            let mut row = columns.iter().map(|column| column[index]);
            let first = row.next().unwrap_or(0.0);
            let value = match aggregation {
                Aggregation::Sum => first + row.sum::<f64>(),
                Aggregation::Max => row.fold(first, f64::max),
                Aggregation::Product => row.fold(first, |total, next| total * next),
                Aggregation::Min => row.fold(first, f64::min),
                Aggregation::Mean => {
                    (first + row.sum::<f64>())
                        / f64::from(u32::try_from(columns.len()).unwrap_or(1))
                }
            };
            value / scale
        })
        .collect::<Vec<_>>();
    from_draws(combined).unwrap_or(Value::Number(0.0))
}

/// Rewrites a flow through one attached behaviour.
///
/// Only the signals a behaviour declares are replaced; the rest travel on
/// untouched, so attaching a timeout does not silently discard the payload size
/// a downstream store needs.
///
/// Both directions are in scope. `signal` is the flow being rewritten, while
/// `demand` and `response` always name the outward and returning flows, so a
/// retry policy can raise demand in proportion to the latency coming back. Each
/// transform reads the flow as it arrived rather than as an earlier transform
/// left it, which keeps a behaviour's transforms independent of the order the
/// catalogue happens to store them in.
#[allow(clippy::too_many_arguments)]
fn apply(
    plan: &Plan,
    mutator: &PreparedMutator,
    flow: BTreeMap<String, Value>,
    counterpart: &BTreeMap<String, Value>,
    direction: Direction,
    config: EvaluationConfig,
    time: f64,
    runtime: &mut crate::squiggle::Runtime,
) -> Result<BTreeMap<String, Value>, EvaluationError> {
    let programs = match direction {
        Direction::Request => &mutator.requests,
        Direction::Response => &mutator.responses,
    };
    if programs.is_empty() {
        return Ok(flow);
    }
    let (request, response) = match direction {
        Direction::Request => (flow.clone(), counterpart.clone()),
        Direction::Response => (counterpart.clone(), flow.clone()),
    };
    let mut scope = plan.globals.clone();
    scope.extend(mutator.properties.clone());
    scope.insert(TIME.to_owned(), Value::Number(time));
    scope.insert(STEP.to_owned(), Value::Number(config.step));
    scope.insert(SIGNAL.to_owned(), Value::Dictionary(flow.clone()));
    scope.insert(REQUEST.to_owned(), Value::Dictionary(request));
    scope.insert(RESPONSE.to_owned(), Value::Dictionary(response));

    let mut rewritten = flow;
    for (signal, program) in programs {
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
    weight: f64,
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
        let value = from_draws(blend(&previous, &next, weight)).unwrap_or(Value::Number(0.0));
        blended.channels.insert(name.clone(), value);
    }
    // A port publishes quantities derived from the channels, so it follows
    // whichever blended channel it names. Values that do not correspond to a
    // channel are constant and are carried through as computed.
    blended.requests = republish(&component.outbound, &blended.channels, &computed.requests);
    blended.responses = republish(&component.inbound, &blended.channels, &computed.responses);
    blended.arriving = computed.arriving.clone();
    blended.returning = computed.returning.clone();
    (blended, moved)
}

/// Re-derives each port's published signals from the blended channels.
///
/// A publication that names a channel outright follows that channel's blended
/// value, so the figure travelling the wire is damped exactly as the quantity it
/// reports is. Anything computed some other way is carried through as evaluated
/// and settles on the next pass.
fn republish(
    ports: &BTreeMap<String, super::compile::PreparedPort>,
    channels: &BTreeMap<String, Value>,
    computed: &BTreeMap<String, BTreeMap<String, Value>>,
) -> BTreeMap<String, BTreeMap<String, Value>> {
    ports
        .iter()
        .map(|(name, port)| {
            let published = port
                .publishes
                .iter()
                .filter_map(|(signal, source, _)| {
                    let value = channels
                        .get(source)
                        .or_else(|| computed.get(name).and_then(|signals| signals.get(signal)))?;
                    Some((signal.clone(), value.clone()))
                })
                .collect::<BTreeMap<_, _>>();
            (name.clone(), published)
        })
        .collect()
}
