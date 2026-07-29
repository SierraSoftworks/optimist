//! The changes an editor can make to a design.
//!
//! A mutation names one entity rather than patching one field. An entity is the
//! unit an engineer actually thinks in — a component, a connection, a shared
//! quantity — and it is the unit that stays coherent on its own: half a
//! component is not a thing anyone means to express.
//!
//! It is also what keeps last-write-wins tolerable. Two people editing
//! different components never contend, and two editing the same one replace it
//! whole rather than interleaving fields into a state neither of them wrote.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::system::{
    Component, ComponentId, Intervention, InterventionId, Relationship, ScaleUnit, ScaleUnitId,
    ScratchpadEntry,
};

/// One change to a design.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mutation {
    /// Adds a shared quantity, or replaces the one with the same name.
    SetScratchpadEntry {
        /// The quantity being defined.
        entry: ScratchpadEntry,
    },
    /// Removes a shared quantity.
    RemoveScratchpadEntry {
        /// Name of the quantity.
        name: String,
    },
    /// Moves a shared quantity before another, or to the end.
    MoveScratchpadEntry {
        /// Name of the quantity to move.
        name: String,
        /// Name to place it before, or none to place it last.
        before: Option<String>,
    },
    /// Adds a component, or replaces the one with the same identifier.
    SetComponent {
        /// The component being defined.
        component: Component,
    },
    /// Removes a component and every relationship touching it.
    RemoveComponent {
        /// Identifier of the component.
        id: ComponentId,
    },
    /// Adds a connection, or replaces the one between the same two components.
    SetRelationship {
        /// The connection being defined.
        relationship: Relationship,
    },
    /// Removes a connection.
    RemoveRelationship {
        /// Component publishing the flow.
        from: ComponentId,
        /// Component receiving it.
        to: ComponentId,
    },
    /// Adds a scale unit, or replaces the one with the same identifier.
    SetScaleUnit {
        /// The scale unit being defined.
        scale_unit: ScaleUnit,
    },
    /// Removes a scale unit.
    RemoveScaleUnit {
        /// Identifier of the scale unit.
        id: ScaleUnitId,
    },
    /// Adds an intervention, or replaces the one with the same identifier.
    SetIntervention {
        /// The intervention being defined.
        intervention: Intervention,
    },
    /// Removes an intervention.
    RemoveIntervention {
        /// Identifier of the intervention.
        id: InterventionId,
    },
}

/// Why a change could not be applied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MutationError {
    /// A connection names a component the design does not contain.
    UnknownComponent {
        /// The identifier that did not resolve.
        id: String,
    },
    /// A connection would leave and arrive at the same component.
    SelfRelationship {
        /// The component on both ends.
        id: String,
    },
    /// A scale unit claims a component another already claims.
    SharedMembership {
        /// The contested component.
        id: String,
    },
    /// A scale unit is enclosed by one the design does not contain.
    UnknownScaleUnit {
        /// The identifier that did not resolve.
        id: String,
    },
    /// The change would remove something that is not there.
    Absent {
        /// What was being removed.
        what: String,
    },
}

impl fmt::Display for MutationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownComponent { id } => {
                write!(formatter, "the design contains no component '{id}'")
            }
            Self::SelfRelationship { id } => write!(
                formatter,
                "'{id}' cannot connect to itself; feedback travels through another component"
            ),
            Self::SharedMembership { id } => write!(
                formatter,
                "'{id}' already belongs to another scale unit; nest the units instead"
            ),
            Self::UnknownScaleUnit { id } => {
                write!(formatter, "the design contains no scale unit '{id}'")
            }
            Self::Absent { what } => write!(formatter, "there is no {what} to remove"),
        }
    }
}

impl std::error::Error for MutationError {}
