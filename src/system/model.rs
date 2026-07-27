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

/// One part of the system being designed.
#[derive(Clone, Debug, Deserialize, Serialize)]
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
}

/// A directed flow from one component's outputs to another's inbound port.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Relationship {
    /// Component publishing the flow.
    pub from: ComponentId,
    /// Component receiving the flow.
    pub to: ComponentId,
    /// Behaviours applied to the flow, in the order they take effect.
    #[serde(default)]
    pub mutators: Vec<super::mutator::AttachedMutator>,
    /// What this connection represents.
    #[serde(default)]
    pub summary: String,
}

/// A complete system design.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
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
}

/// One shared quantity available throughout a model.
#[derive(Clone, Debug, Deserialize, Serialize)]
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
}
