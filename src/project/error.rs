use thiserror::Error;

use crate::{
    domain::{EdgeIdError, NodeError, ProjectId},
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
    /// Creating or accessing the project's isolated graph repository failed.
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}
