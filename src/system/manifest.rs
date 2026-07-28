//! The persisted shape of a component type definition.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A stable identifier for a component type, unique within a catalogue.
///
/// Identifiers are lower-case words joined by hyphens, so they read the same in
/// a manifest file name, a YAML document, and a URL.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(transparent)]
pub struct ComponentTypeId(String);

impl ComponentTypeId {
    /// Borrows the identifier's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ComponentTypeId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<&str> for ComponentTypeId {
    fn from(id: &str) -> Self {
        Self(id.to_owned())
    }
}

/// How many relationships a port accepts.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PortArity {
    /// The port accepts no more than one relationship.
    One,
    /// The port accepts any number of relationships, including none.
    #[default]
    Many,
}

/// One side of a component's connectivity.
///
/// A port is a named place relationships attach, and the unit in which a
/// component's traffic is separated. A read-through cache declares one inbound
/// port for lookups and two outbound ports, one for the hits it serves itself
/// and one for the misses it forwards, so the two paths can be sized apart
/// instead of being averaged into a single figure that describes neither.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Port {
    /// How many relationships may attach here.
    #[serde(default)]
    pub arity: PortArity,
    /// What attaching to this port means, in the author's terms.
    #[serde(default)]
    pub summary: String,
    /// Squiggle source for each signal this port publishes, keyed by signal name.
    ///
    /// On an inbound port these are the responses sent back to callers, and only
    /// signals that travel backward may appear. On an outbound port they are the
    /// requests sent to dependencies, and only signals that travel forward may.
    ///
    /// Expressions may read the component's properties and channels, so a port
    /// publishes quantities the component has already worked out rather than
    /// recomputing them. They may not read the port's own published values,
    /// which is what keeps a component from depending on what it is saying.
    #[serde(default)]
    pub publishes: BTreeMap<String, String>,
}

/// The named places relationships attach to a component.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Ports {
    /// Ports callers attach to, carrying requests in and responses back.
    #[serde(rename = "in", default)]
    pub inbound: BTreeMap<String, Port>,
    /// Ports dependencies attach to, carrying requests out and responses back.
    #[serde(rename = "out", default)]
    pub outbound: BTreeMap<String, Port>,
}

impl Ports {
    /// Returns the only port on a side, when a relationship names none.
    ///
    /// A model that does not mention ports is unambiguous exactly when there is
    /// one to choose, so this resolves the common case without letting a type
    /// with several ports be wired by guesswork.
    pub fn sole(ports: &BTreeMap<String, Port>) -> Option<&String> {
        let mut names = ports.keys();
        let only = names.next()?;
        names.next().is_none().then_some(only)
    }
}

/// An intrinsic fact an author supplies about a component.
///
/// Properties are the measurable surface of a component: the numbers an engineer
/// can look up, benchmark, or estimate. Everything else the model reports is
/// derived from them.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Property {
    /// Unit annotation the authored value must satisfy, such as `op/s`.
    pub unit: String,
    /// What the property measures and how an author should obtain it.
    #[serde(default)]
    pub summary: String,
    /// Squiggle source used when an author supplies no value.
    ///
    /// A property without a default must be supplied, because no sensible
    /// stand-in exists for a quantity that varies by orders of magnitude between
    /// deployments.
    #[serde(default)]
    pub default: Option<String>,
}

impl Property {
    /// Reports whether an author must supply this property.
    pub fn is_required(&self) -> bool {
        self.default.is_none()
    }
}

/// A quantity derived from properties, inbound flows, and prior state.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Channel {
    /// Unit annotation the derived quantity carries.
    pub unit: String,
    /// What the channel represents and which law produces it.
    #[serde(default)]
    pub summary: String,
    /// Squiggle source evaluated over sample sets to produce the quantity.
    pub expression: String,
}

/// A resource limit and the demand placed against it.
///
/// Constraints are what the engine ranks. Utilisation is demand over limit, and
/// the share of draws in which demand meets or exceeds the limit is the
/// probability that this constraint binds.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Constraint {
    /// What saturating this constraint does to the system.
    #[serde(default)]
    pub summary: String,
    /// Squiggle source for the quantity consuming the resource.
    pub demand: String,
    /// Squiggle source for the quantity available.
    pub limit: String,
}

/// A complete, validated definition of one kind of component.
///
/// Construct through [`ComponentType::parse`] so that the invariants the
/// evaluator relies on are checked once, at load time, rather than rediscovered
/// per draw inside a solver loop.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ComponentType {
    /// Stable identifier used by components that adopt this type.
    pub id: ComponentTypeId,
    /// Human-readable name shown when choosing a component kind.
    pub name: String,
    /// What this kind of component models and when to reach for it.
    #[serde(default)]
    pub summary: String,
    /// The named places relationships attach, and what each publishes.
    #[serde(default)]
    pub ports: Ports,
    /// Intrinsic facts an author supplies.
    #[serde(default)]
    pub properties: BTreeMap<String, Property>,
    /// Quantities derived from properties, inbound flows, and prior state.
    #[serde(default)]
    pub channels: BTreeMap<String, Channel>,
    /// Resource limits this component can saturate.
    #[serde(default)]
    pub constraints: BTreeMap<String, Constraint>,
}
