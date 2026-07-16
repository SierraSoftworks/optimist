use thiserror::Error;

use crate::domain::{EstimateAddress, FormulaError};

/// Failures returned by project-scoped Fermi component formula operations.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum FormulaCommandError {
    /// A target component address belongs to another isolated project.
    #[error("formula address {0} belongs to another project")]
    CrossProjectAddress(EstimateAddress),
    /// Root estimate addresses are owned by primitive estimate commands.
    #[error("formula address {0} must identify a nested component")]
    RootAddress(EstimateAddress),
    /// The component's root does not resolve to a stored primitive estimate.
    #[error("formula component root {0} does not identify a primitive estimate")]
    MissingPrimitiveRoot(EstimateAddress),
    /// A nested component's immediate parent formula is absent.
    #[error("formula component parent {0} does not exist")]
    MissingParent(EstimateAddress),
    /// Stored graph data repeats an owner-local primitive estimate identity.
    #[error("primitive estimate address {0} occurs more than once")]
    DuplicatePrimitive(EstimateAddress),
    /// A project-defined cost dimension cannot be represented by current unit algebra.
    #[error("cost dimension {0:?} is not a valid formula unit identifier")]
    InvalidPrimitiveUnit(String),
    /// The requested component formula does not exist.
    #[error("formula component {0} does not exist")]
    NotFound(EstimateAddress),
    /// Another formula still references the requested component.
    #[error("formula component {address} is referenced by {dependent}")]
    Referenced {
        /// Component requested for removal.
        address: EstimateAddress,
        /// Formula whose compiled dependencies include the component.
        dependent: Box<EstimateAddress>,
    },
    /// A nested descendant prevents removal of its parent component.
    #[error("formula component {address} has descendant {descendant}")]
    HasDescendant {
        /// Parent requested for removal.
        address: EstimateAddress,
        /// Existing nested descendant.
        descendant: Box<EstimateAddress>,
    },
    /// A formula document command used an older document revision.
    #[error("formula document revision conflict: expected {expected}, current {current}")]
    RevisionConflict {
        /// Revision supplied by the authoring client.
        expected: u64,
        /// Revision currently stored by the project.
        current: u64,
    },
    /// The formula document cannot represent another mutation.
    #[error("project formula document has exhausted its revision space")]
    RevisionSpaceExhausted,
    /// Formula graph structure or dimensional validation failed.
    #[error(transparent)]
    Formula(#[from] FormulaError),
}
