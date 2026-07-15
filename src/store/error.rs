use thiserror::Error;

use crate::domain::{EdgeError, EntityId, NodeKind};

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RepositoryError {
    #[error("entity ID {0} already exists in this project")]
    DuplicateEntity(EntityId),
    #[error("the normalized name or alias {0:?} already exists in this project")]
    DuplicateName(String),
    #[error("entity {id} stores normalized name {actual:?}, expected {expected:?}")]
    InvalidNormalizedName {
        id: EntityId,
        actual: String,
        expected: String,
    },
    #[error("entity {0} contains an empty or duplicate name/alias")]
    InvalidNameClaim(EntityId),
    #[error("entity {0} does not exist in this project")]
    MissingEntity(EntityId),
    #[error("entity {id} is {actual:?}, but the edge declares it as {declared:?}")]
    EndpointKindMismatch {
        id: EntityId,
        actual: NodeKind,
        declared: NodeKind,
    },
    #[error("edge {0} already exists in this project")]
    DuplicateEdge(String),
    #[error("edge {0} does not exist in this project")]
    MissingEdge(String),
    #[error("entity {0} cannot be deleted while it has incident edges")]
    EntityHasEdges(EntityId),
    #[error("the project has exhausted its entity identifier space")]
    IdentifierSpaceExhausted,
    #[error(transparent)]
    InvalidEdge(#[from] EdgeError),
}
