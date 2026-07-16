use thiserror::Error;

use crate::domain::{EdgeId, EntityId};

/// Failures returned by node and edge presentation metadata updates.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AggregateUpdateError {
    /// A node update used an older aggregate revision.
    #[error("node {id} revision conflict: expected {expected}, current {current}")]
    NodeRevisionConflict {
        /// Project-local node identity.
        id: EntityId,
        /// Revision supplied by the caller.
        expected: u64,
        /// Revision currently stored by the project.
        current: u64,
    },
    /// An edge update used an older aggregate revision.
    #[error("edge {id} revision conflict: expected {expected}, current {current}")]
    EdgeRevisionConflict {
        /// Canonical structural edge identity.
        id: EdgeId,
        /// Revision supplied by the caller.
        expected: u64,
        /// Revision currently stored by the project.
        current: u64,
    },
    /// A node aggregate cannot represent another metadata update.
    #[error("node {0} has exhausted its revision space")]
    NodeRevisionSpaceExhausted(EntityId),
}
