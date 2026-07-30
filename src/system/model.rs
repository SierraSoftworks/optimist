//! Instances of component types wired together into a system.
//!
//! A component type describes a *kind* of part; a model is the particular
//! arrangement being designed. Each component adopts a type and supplies values
//! for its properties, and each relationship declares that one component's
//! outputs become another's inbound flow.
//!
//! # The scratchpad
//!
//! Quantities shared across a design live in a scratchpad rather than being
//! repeated at every component that needs them. A record size, a global request
//! rate, or a peak-to-mean ratio is one fact about the system, and stating it
//! once means an experiment can change it once. Scratchpad entries are ordinary
//! Squiggle bindings evaluated before anything else, so later entries may build
//! on earlier ones and any component may refer to any of them.
//!
//! This is also the surface an intervention acts on. Comparing designs is then a
//! matter of rebinding a named quantity and re-running, rather than editing the
//! structure of the model and hoping the two versions stayed comparable.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::manifest::ComponentTypeId;

/// A stable identifier for a component within one model.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(transparent)]
pub struct ComponentId(String);

impl ComponentId {
    /// Creates an identifier from its text.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrows the identifier's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ComponentId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Where a component sits on the diagram.
///
/// Layout is stored with the design because it carries meaning. Somebody who
/// arranges a diagram is saying how the system is best read — demand at the top,
/// the dependency that everything waits on in the middle — and that judgement is
/// worth keeping and worth reviewing alongside the model it describes.
///
/// Absent until somebody moves the component, so a design nobody has arranged is
/// laid out automatically rather than pinned to whatever an algorithm produced
/// the first time it was opened.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    /// Horizontal position, in the diagram's own units.
    pub x: f64,
    /// Vertical position, in the diagram's own units.
    pub y: f64,
}

/// One part of the system being designed.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    /// Identifier unique within the model.
    pub id: ComponentId,
    /// Human-readable name.
    pub name: String,
    /// The component type this instance adopts.
    #[serde(rename = "type")]
    pub component_type: ComponentTypeId,
    /// Squiggle source for each property the type declares, keyed by name.
    #[serde(default)]
    pub properties: BTreeMap<String, String>,
    /// Where this sits on the diagram, once somebody has placed it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
}

/// A directed flow between one component's outbound port and another's inbound.
///
/// A relationship is a wire, not a one-way pipe. Requests travel from `from` to
/// `to`, and the response travels back along the same relationship, so a call
/// graph is drawn once rather than once per direction.
///
/// It is also a queue. Work offered faster than it can be taken waits somewhere,
/// and that somewhere is real whether or not anybody drew it: a socket buffer, a
/// listen backlog, a connection pool's wait list. Modelling the wire as a queue
/// puts that buffering in one place instead of asking every component type to
/// reimplement it, and makes the queues a design already contains visible
/// without anybody having to add them.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    /// Component publishing the flow.
    pub from: ComponentId,
    /// Outbound port on `from` that this relationship leaves by.
    ///
    /// Omitted when the type declares exactly one outbound port, which is the
    /// common case and leaves simple designs free of wiring detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_port: Option<String>,
    /// Component receiving the flow.
    pub to: ComponentId,
    /// Inbound port on `to` that this relationship arrives at.
    ///
    /// Omitted when the type declares exactly one inbound port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_port: Option<String>,
    /// Squiggle source for how many operations may wait on this wire.
    ///
    /// Defaults to a hundred, which is the order of a network link between two
    /// services: socket buffers and a listen backlog, deep enough to ride out a
    /// brief burst and shallow enough that sustained overload is felt rather
    /// than hidden. An in-process call is nearer one, and a broker with disk
    /// backing is far larger.
    ///
    /// Depth is not free. A queue absorbs a burst by making the caller wait for
    /// it, so a generous buffer converts a capacity problem into a latency one,
    /// and a caller with a deadline turns that latency back into failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity: Option<String>,
    /// Squiggle source for how fast this wire carries bytes.
    ///
    /// Unlimited by default, which keeps a relationship a pure operation queue
    /// unless an author says otherwise. Say otherwise wherever the link is a
    /// real one: a network interface, an inter-region path, a disk bus. A design
    /// whose operation rates all fit can still be bound by the bytes those
    /// operations carry, and that is the limit nobody draws.
    ///
    /// The bytes are the request and the reply together, at the sizes the
    /// behaviours on the wire declare, so batching several calls into one leaves
    /// this unchanged while dividing the operation rate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bandwidth: Option<String>,
    /// Behaviours applied to the flow, in the order they take effect.
    #[serde(default)]
    pub mutators: Vec<super::mutator::AttachedMutator>,
    /// What this connection represents.
    #[serde(default)]
    pub summary: String,
}

/// Operations a relationship holds when its author says nothing.
pub const DEFAULT_LINK_CAPACITY: &str = "100";

/// Bytes per second a relationship carries when its author says nothing.
///
/// Unbounded, because a link speed nobody stated is a link speed nobody meant to
/// constrain, and inventing one would put a limit into a design that its author
/// never wrote down.
pub const DEFAULT_LINK_BANDWIDTH: &str = "infinity";

impl Relationship {
    /// Borrows the authored queue depth, or the default network link.
    pub fn capacity_source(&self) -> &str {
        self.capacity.as_deref().unwrap_or(DEFAULT_LINK_CAPACITY)
    }

    /// Borrows the authored link speed, or an unlimited one.
    pub fn bandwidth_source(&self) -> &str {
        self.bandwidth.as_deref().unwrap_or(DEFAULT_LINK_BANDWIDTH)
    }
}

/// A complete system design.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SystemModel {
    /// Shared quantities available to every component, in evaluation order.
    #[serde(default)]
    pub scratchpad: Vec<ScratchpadEntry>,
    /// The parts of the system.
    #[serde(default)]
    pub components: Vec<Component>,
    /// How those parts are wired together.
    #[serde(default)]
    pub relationships: Vec<Relationship>,
    /// Boundaries within which components are replicated together.
    #[serde(default)]
    pub scale_units: Vec<super::scale_unit::ScaleUnit>,
    /// Proposed changes, expressed as rebindings of shared quantities.
    #[serde(default)]
    pub interventions: Vec<super::intervention::Intervention>,
}

/// One shared quantity available throughout a model.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScratchpadEntry {
    /// Binding name referenced by component properties.
    pub name: String,
    /// Squiggle source producing the value.
    pub expression: String,
    /// Unit annotation the value carries.
    #[serde(default)]
    pub unit: Option<String>,
    /// What the quantity represents and where its value came from.
    #[serde(default)]
    pub summary: String,
}

impl SystemModel {
    /// Returns the relationships arriving at `component`, in model order.
    pub fn inbound_to(&self, component: &ComponentId) -> Vec<&Relationship> {
        self.relationships
            .iter()
            .filter(|relationship| &relationship.to == component)
            .collect()
    }

    /// Returns the relationships departing from `component`, in model order.
    ///
    /// These are the dependencies `component` calls, and therefore the
    /// relationships along which responses travel back to it.
    pub fn outbound_from(&self, component: &ComponentId) -> Vec<&Relationship> {
        self.relationships
            .iter()
            .filter(|relationship| &relationship.from == component)
            .collect()
    }

    /// Returns the components publishing into `component`, in model order.
    pub fn upstream_of(&self, component: &ComponentId) -> Vec<&ComponentId> {
        self.relationships
            .iter()
            .filter(|relationship| &relationship.to == component)
            .map(|relationship| &relationship.from)
            .collect()
    }

    /// Returns every component identifier declared by the model.
    pub fn identifiers(&self) -> BTreeSet<&ComponentId> {
        self.components
            .iter()
            .map(|component| &component.id)
            .collect()
    }

    /// Sorts the model into the order persistence reads and writes it in.
    ///
    /// Ordering carries no meaning: the graph is defined by relationships, and
    /// the solver visits every component on every pass. It is not, however,
    /// inert. A pass updates components as it goes, so one evaluated after its
    /// upstream sees that upstream's new value while one evaluated before sees
    /// the previous pass's. Order therefore changes the path an iteration takes
    /// to its fixed point, and with a finite tolerance it changes the last
    /// digits of where that iteration stops.
    ///
    /// Fixing a canonical order makes a design reproducible regardless of how it
    /// was assembled or which files happened to be read first. A model that has
    /// been through persistence is already in this order.
    pub fn canonicalise(mut self) -> Self {
        self.components
            .sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        self.relationships.sort_by(|left, right| {
            left.from
                .as_str()
                .cmp(right.from.as_str())
                .then(left.to.as_str().cmp(right.to.as_str()))
        });
        self.scale_units
            .sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        self
    }

    /// Borrows one of the model's interventions.
    pub(super) fn intervention(
        &self,
        id: &super::intervention::InterventionId,
    ) -> Result<&super::intervention::Intervention, super::evaluate::EvaluationError> {
        self.interventions
            .iter()
            .find(|intervention| &intervention.id == id)
            .ok_or_else(|| super::evaluate::EvaluationError::UnknownIntervention {
                intervention: id.to_string(),
            })
    }
}
