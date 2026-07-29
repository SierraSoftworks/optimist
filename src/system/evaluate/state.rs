//! What a solve produces: per-component results and per-wire backlogs.

use std::collections::BTreeMap;

use crate::{squiggle::Value, system::model::ComponentId};

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
    /// Bytes per second crossing the wire, request and reply together.
    pub transfer: Value,
    /// Bytes per second the wire can carry.
    pub bandwidth: Value,
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
            transfer: Value::Number(0.0),
            bandwidth: Value::Number(f64::INFINITY),
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
    /// What was still moving when it gave up, where it did not settle.
    pub unsettled: Option<Unsettled>,
    /// Passes taken before settling or reaching the cap.
    pub iterations: usize,
    /// Largest relative movement in the final pass.
    pub movement: f64,
}

/// The quantity that kept a step from settling.
///
/// A step that does not settle is a result rather than a failure, but "nothing
/// settled" sends an author looking through a whole design. Naming the quantity
/// that was still moving, and how fast, points at the loop that is not closing.
#[derive(Clone, Debug)]
pub struct Unsettled {
    /// Component owning the quantity that was still moving furthest.
    pub component: ComponentId,
    /// That component's channel which was still moving furthest.
    pub channel: String,
    /// How far it moved on the last pass, relative to its own magnitude.
    pub movement: f64,
    /// Whether the iterate had stopped getting closer, rather than merely run
    /// out of passes.
    ///
    /// The two call for different answers. An iterate still closing in wants a
    /// higher cap; one that has stopped has no steady state to find at this load,
    /// and raising the cap only makes the same answer take longer to arrive.
    pub stalled: bool,
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
