use thiserror::Error;

use crate::domain::{EdgeError, EntityId, NodeKind};

/// Result type shared by repository implementations.
pub type RepositoryResult<T> = Result<T, RepositoryError>;

/// Backend-independent failures for validated graph persistence.
///
/// Keeping semantic conflicts separate from datastore failures lets HTTP/CLI layers
/// offer corrective advice instead of reporting every rejection as an internal error.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum RepositoryError {
    /// The project's entity namespace already contains this short ID.
    #[error("entity ID {0} already exists in this project")]
    DuplicateEntity(EntityId),
    /// A node name or alias collides after canonical normalization.
    #[error("the normalized name or alias {0:?} already exists in this project")]
    DuplicateName(String),
    /// Serialized data contains a canonical name inconsistent with its source name.
    #[error("entity {id} stores normalized name {actual:?}, expected {expected:?}")]
    InvalidNormalizedName {
        /// Entity containing the corrupt value.
        id: EntityId,
        /// Canonical value found in the payload.
        actual: String,
        /// Canonical value derived from the source name.
        expected: String,
    },
    /// One node claims an empty alias or repeats a name/alias internally.
    #[error("entity {0} contains an empty or duplicate name/alias")]
    InvalidNameClaim(EntityId),
    /// A requested entity or edge endpoint is absent from this project.
    #[error("entity {0} does not exist in this project")]
    MissingEntity(EntityId),
    /// An edge's declared endpoint kind disagrees with the stored node payload.
    #[error("entity {id} is {actual:?}, but the edge declares it as {declared:?}")]
    EndpointKindMismatch {
        /// Endpoint whose declaration is inconsistent.
        id: EntityId,
        /// Kind derived from the stored node payload.
        actual: NodeKind,
        /// Kind supplied in the edge payload.
        declared: NodeKind,
    },
    /// The canonical `(source, kind, destination)` edge already exists.
    #[error("edge {0} already exists in this project")]
    DuplicateEdge(String),
    /// The requested canonical edge key does not exist.
    #[error("edge {0} does not exist in this project")]
    MissingEdge(String),
    /// Deletion would leave dangling incoming or outgoing edge references.
    #[error("entity {0} cannot be deleted while it has incident edges")]
    EntityHasEdges(EntityId),
    /// A project's monotonic entity counter cannot allocate another value.
    #[error("the project has exhausted its entity identifier space")]
    IdentifierSpaceExhausted,
    /// IndraDB or another backing datastore rejected an operation.
    #[error("the graph datastore failed: {0}")]
    Datastore(String),
    /// Stored JSON cannot be decoded into the validated aggregate type.
    #[error("the graph contains an invalid serialized payload: {0}")]
    InvalidPayload(String),
    /// Edge endpoint or symmetry validation failed.
    #[error(transparent)]
    InvalidEdge(#[from] EdgeError),
}
