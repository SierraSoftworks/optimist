use thiserror::Error;

use crate::domain::{
    EstimateAddress, EstimateError, EstimateSlot, EstimateSlotError, FermiAssessmentError,
    FermiEstimateError,
};

/// Failures returned by primitive estimate authoring and lookup operations.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum EstimateCommandError {
    /// An estimate command address belongs to another isolated project.
    #[error("estimate address {0} belongs to another project")]
    CrossProjectAddress(EstimateAddress),
    /// Nested Fermi components are not primitive estimate roots.
    #[error("estimate address {0} identifies a nested component")]
    NestedAddress(EstimateAddress),
    /// The selected semantic slot does not exist on the addressed owner payload.
    #[error("estimate slot {slot:?} is invalid for owner {address}")]
    InvalidSlot {
        /// Address naming the node or edge owner.
        address: EstimateAddress,
        /// Slot rejected by the owner's typed payload.
        slot: EstimateSlot,
    },
    /// The requested estimate ID is already used by another owner-local slot.
    #[error("estimate ID in {0} is already used by another owner slot")]
    IdentifierConflict(EstimateAddress),
    /// The selected slot already contains an estimate with a different ID.
    #[error("estimate slot {slot:?} is already occupied in {address}")]
    SlotOccupied {
        /// Address containing the proposed owner-local ID.
        address: EstimateAddress,
        /// Occupied semantic owner field.
        slot: EstimateSlot,
    },
    /// No primitive estimate exists at the requested address.
    #[error("estimate {0} does not exist")]
    NotFound(EstimateAddress),
    /// Removing this estimate would invalidate its mandatory edge payload.
    #[error("estimate {address} in required slot {slot:?} cannot be removed")]
    Required {
        /// Existing estimate address.
        address: EstimateAddress,
        /// Mandatory typed edge field.
        slot: EstimateSlot,
    },
    /// The project dependence document still references this estimate.
    #[error("estimate {0} is referenced by the project dependence model")]
    ReferencedByDependence(EstimateAddress),
    /// A stored Fermi component is rooted under or references this estimate.
    #[error("estimate {address} is used by formula component {formula}")]
    ReferencedByFormula {
        /// Primitive estimate requested for removal.
        address: EstimateAddress,
        /// Formula rooted under or referencing the estimate.
        formula: Box<EstimateAddress>,
    },
    /// A primitive distribution does not satisfy its slot's typed support.
    #[error(transparent)]
    Estimate(#[from] EstimateError),
    /// Estimate slot input is empty or otherwise invalid.
    #[error(transparent)]
    Slot(#[from] EstimateSlotError),
    /// Persisted Fermi authoring metadata is invalid.
    #[error(transparent)]
    Fermi(#[from] FermiEstimateError),
    /// The canonical formula could not be validated or sampled for this slot.
    #[error(transparent)]
    FermiAssessment(#[from] FermiAssessmentError),
    /// Assessment completed without a usable primitive recommendation.
    #[error("the Fermi equation did not produce a persistable distribution recommendation")]
    UnavailableFermiRecommendation,
    /// A node or estimate aggregate revision cannot represent another update.
    #[error("owner of estimate {0} has exhausted its revision space")]
    RevisionSpaceExhausted(EstimateAddress),
}
