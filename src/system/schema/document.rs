//! The documents a design directory is made of.

use serde::{Deserialize, Serialize};

use crate::system::{
    intervention::Intervention,
    model::{Component, ComponentId, ScratchpadEntry},
    mutator::AttachedMutator,
    scale_unit::ScaleUnit,
};

/// The design-wide document, stored as `_system.yaml`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SystemDocument {
    /// Schema version this directory was written against.
    pub schema_version: u32,
    /// Human-readable name for the design.
    pub name: String,
    /// What the design is for.
    #[serde(default)]
    pub summary: String,
    /// Quantities shared across the design, in evaluation order.
    #[serde(default)]
    pub scratchpad: Vec<ScratchpadEntry>,
    /// Boundaries within which components are replicated together.
    #[serde(default)]
    pub scale_units: Vec<ScaleUnit>,
    /// Proposed changes, expressed as rebindings of shared quantities.
    #[serde(default)]
    pub interventions: Vec<Intervention>,
}

/// One component and the relationships leaving it.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentDocument {
    /// The component itself.
    #[serde(flatten)]
    pub component: Component,
    /// Relationships this component publishes onto.
    #[serde(default)]
    pub outgoing: Vec<OutgoingRelationship>,
}

/// A relationship stored with the component it leaves.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OutgoingRelationship {
    /// Outbound port on the owning component this relationship leaves by.
    ///
    /// Omitted when the type declares exactly one outbound port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_port: Option<String>,
    /// Component receiving the flow.
    pub to: ComponentId,
    /// Inbound port on the receiving component this relationship arrives at.
    ///
    /// Omitted when the type declares exactly one inbound port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_port: Option<String>,
    /// Squiggle source for how many operations may wait on this wire.
    ///
    /// Omitted to accept the default network link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity: Option<String>,
    /// Squiggle source for how fast this wire carries bytes.
    ///
    /// Omitted to leave the link unlimited, which is right until somebody says
    /// what the link actually is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bandwidth: Option<String>,
    /// Behaviours applied to the flow, in the order they take effect.
    #[serde(default)]
    pub mutators: Vec<AttachedMutator>,
    /// What this connection represents.
    #[serde(default)]
    pub summary: String,
}
