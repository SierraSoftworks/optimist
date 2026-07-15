use thiserror::Error;

use crate::{
    domain::{EdgeId, EdgeIdError, NodeError, ObservationError, ProjectId},
    store::RepositoryError,
};

/// Failures which prevent project lifecycle operations from completing.
///
/// HTTP handlers map these variants to stable status/code/advice responses, while
/// CLI callers wrap them in `human_errors` at the process boundary.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ProjectError {
    /// The proposed name contains no text after normalization.
    #[error("a project name cannot be empty")]
    EmptyName,
    /// Another project already claims the normalized form of this name.
    #[error("a project named {0:?} already exists")]
    DuplicateName(String),
    /// No project exists for the requested server-local ID.
    #[error("project {0} does not exist")]
    NotFound(ProjectId),
    /// The server's monotonic project-ID counter cannot allocate another value.
    #[error("the server has exhausted its project identifier space")]
    IdentifierSpaceExhausted,
    /// A command was based on an older project revision than the current graph.
    #[error("project revision conflict: expected {expected}, current {current}")]
    RevisionConflict {
        /// Revision supplied by the client before it prepared the mutation.
        expected: u64,
        /// Revision currently stored by the serialized project executor.
        current: u64,
    },
    /// The project revision counter cannot represent another committed mutation.
    #[error("project {0} has exhausted its revision space")]
    RevisionSpaceExhausted(ProjectId),
    /// The requested node aggregate failed local construction validation.
    #[error(transparent)]
    Node(#[from] NodeError),
    /// An external edge ID does not use the canonical tuple representation.
    #[error(transparent)]
    EdgeId(#[from] EdgeIdError),
    /// The selected edge does not own a measurement observation series.
    #[error("edge {0} is not a measurement edge")]
    NotMeasurementEdge(EdgeId),
    /// A reading's unit disagrees with the source metric definition.
    #[error("observation unit {actual:?} does not match metric unit {expected:?}")]
    ObservationUnitMismatch {
        /// Unit declared by the source metric node.
        expected: String,
        /// Unit supplied with the new observation.
        actual: String,
    },
    /// The edge aggregate cannot represent another revision.
    #[error("edge {0} has exhausted its revision space")]
    EdgeRevisionSpaceExhausted(EdgeId),
    /// Observation validation or immutable correction semantics failed.
    #[error(transparent)]
    Observation(#[from] ObservationError),
    /// Creating or accessing the project's isolated graph repository failed.
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}
