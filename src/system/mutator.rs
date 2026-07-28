//! Behaviours attached to a relationship rather than to a component.
//!
//! # Why these are not components
//!
//! A retry policy, a timeout, or a batching window is not a place work goes; it
//! is a rule about how work travels. Modelling them as components would put a
//! box on the diagram for every policy and obscure the shape of the system,
//! while modelling them as component properties would mean every component type
//! reimplementing the same rules.
//!
//! A mutator therefore sits on a relationship and transforms the signals passing
//! along it. It sees the flow and its own settings, never the components on
//! either end, which is what lets one definition apply to any connection.
//!
//! # Both directions
//!
//! A relationship carries demand downstream and a response back, and a mutator
//! can rewrite either. Both directions are in scope from either transform, so a
//! behaviour can react to what is coming back: a retry policy raises demand
//! precisely when the response says attempts are timing out, which is the
//! feedback that turns a slow dependency into an overloaded one.
//!
//! # Composition
//!
//! Mutators apply in the order they are declared, each transforming what the one
//! before it produced. Order is a modelling decision with real consequences: a
//! timeout inside a retry bounds each attempt, while a timeout outside one
//! bounds the whole sequence including the waiting between attempts. Writing the
//! order down makes that choice explicit instead of leaving it to be inferred
//! from prose.
//!
//! # Amplification
//!
//! The reason these belong in a capacity model at all is that several of them
//! change how much demand arrives downstream. A retry policy multiplies request
//! rate by the expected number of attempts, a fan-out multiplies it by the
//! branch count, and batching divides it while multiplying payload. Demand
//! amplification is invisible in a diagram and is a common way for a design to
//! be wrong by an order of magnitude.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A stable identifier for a mutator definition.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(transparent)]
pub struct MutatorId(String);

impl MutatorId {
    /// Creates an identifier from its text.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrows the identifier's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MutatorId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// How one signal is rewritten as it passes through a mutator.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Transform {
    /// Unit annotation the rewritten signal carries.
    pub unit: String,
    /// What the transform does to the flow and why.
    #[serde(default)]
    pub summary: String,
    /// Squiggle source producing the rewritten value.
    pub expression: String,
}

/// A declarative definition of one relationship behaviour.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Mutator {
    /// Stable identifier used when attaching this behaviour.
    pub id: MutatorId,
    /// Human-readable name.
    pub name: String,
    /// What the behaviour does and when to reach for it.
    #[serde(default)]
    pub summary: String,
    /// Settings an author supplies when attaching it.
    #[serde(default)]
    pub properties: BTreeMap<String, super::manifest::Property>,
    /// Signals this behaviour rewrites on the way downstream, keyed by signal.
    ///
    /// These act on the request: what the caller is asking its dependency for.
    #[serde(default, alias = "transforms")]
    pub requests: BTreeMap<String, Transform>,
    /// Signals this behaviour rewrites on the way back, keyed by signal name.
    ///
    /// These act on the response: what the dependency reports to its caller. A
    /// timeout belongs here, because it bounds what the caller waits for and
    /// turns the tail it cut off into failure rather than changing what the
    /// dependency was asked to do.
    #[serde(default, alias = "feedback")]
    pub responses: BTreeMap<String, Transform>,
}

/// One behaviour attached to a particular relationship.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AttachedMutator {
    /// The behaviour being attached.
    #[serde(rename = "type")]
    pub mutator: MutatorId,
    /// Squiggle source for each setting the behaviour declares.
    #[serde(default)]
    pub properties: BTreeMap<String, String>,
}
