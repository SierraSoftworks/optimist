//! The shapes a resolved model takes before the solver runs.

use std::collections::BTreeMap;

use crate::{
    squiggle::{Value, ast::Program},
    system::{evaluate::LinkId, manifest::ComponentType, model::ComponentId, signal::Signal},
};

/// One relationship seen from a component, with its behaviours resolved.
///
/// The same relationship appears once on each end: on the caller's outbound port
/// and on the callee's inbound port. Both carry the same behaviours, because a
/// mutator sits on the wire and acts on traffic in both directions.
pub(crate) struct PreparedLink {
    pub(crate) peer: ComponentId,
    /// The port on the peer this relationship attaches to.
    pub(crate) peer_port: String,
    /// Which relationship this is, for finding its backlog between steps.
    pub(crate) id: LinkId,
    /// Operations that may wait on this wire.
    pub(crate) capacity: Value,
    /// Bytes per second this wire carries.
    pub(crate) bandwidth: Value,
    pub(crate) mutators: Vec<PreparedMutator>,
}

/// One named attachment point, with its links and published expressions.
pub(crate) struct PreparedPort {
    pub(crate) links: Vec<PreparedLink>,
    /// Signals this port publishes, as signal name, source text, and program.
    ///
    /// The source is kept so that a publication naming a channel outright can
    /// follow that channel's blended value, which keeps the published figure
    /// inside the relaxation's damping rather than jumping ahead of it.
    pub(crate) publishes: Vec<(String, String, Program)>,
}

/// One behaviour attached to a relationship, ready to apply.
pub(crate) struct PreparedMutator {
    pub(crate) id: String,
    pub(crate) properties: BTreeMap<String, Value>,
    pub(crate) requests: Vec<(String, Program)>,
    pub(crate) responses: Vec<(String, Program)>,
}

/// One component resolved against its type and ready to evaluate.
pub(crate) struct PreparedComponent {
    pub(crate) id: ComponentId,
    pub(crate) component_type: ComponentType,
    pub(crate) properties: BTreeMap<String, Value>,
    pub(crate) channels: Vec<(String, Program)>,
    pub(crate) constraints: BTreeMap<String, (Program, Program)>,
    /// Ports callers attach to, receiving requests and publishing responses.
    pub(crate) inbound: BTreeMap<String, PreparedPort>,
    /// Ports dependencies attach to, publishing requests and receiving responses.
    pub(crate) outbound: BTreeMap<String, PreparedPort>,
    /// Replicas of this component across every enclosing scale unit.
    pub(crate) replicas: f64,
    /// Share of the model's demand one replica serves.
    pub(crate) share: f64,
}

/// A whole model resolved and ready to solve.
pub(crate) struct Plan {
    pub(crate) components: Vec<PreparedComponent>,
    pub(crate) globals: BTreeMap<String, Value>,
    pub(crate) signals: BTreeMap<String, Signal>,
}
