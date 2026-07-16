use thiserror::Error;

use crate::domain::EntityId;

/// Failures returned by factor/outcome evidence lifecycle commands.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum EvidenceCommandError {
    /// Evidence can only be embedded in factors and outcomes.
    #[error("node {0} does not own qualitative evidence")]
    InvalidOwner(EntityId),
    /// Evidence summaries must contain visible text.
    #[error("an evidence summary cannot be empty")]
    EmptySummary,
    /// No evidence record with the requested node-local identity exists.
    #[error("evidence {evidence_id} does not exist on node {node}")]
    NotFound {
        /// Node expected to own the evidence record.
        node: EntityId,
        /// Node-local evidence identity requested by the caller.
        evidence_id: u64,
    },
    /// An evidence replacement or deletion used an older record revision.
    #[error(
        "evidence {evidence_id} on node {node} revision conflict: expected {expected}, current {current}"
    )]
    RevisionConflict {
        /// Node which owns the evidence record.
        node: EntityId,
        /// Node-local evidence identity.
        evidence_id: u64,
        /// Revision supplied by the caller.
        expected: u64,
        /// Revision currently stored in the node payload.
        current: u64,
    },
    /// The owning node revision cannot allocate another evidence identity.
    #[error("node {0} has exhausted its evidence identifier space")]
    IdentifierSpaceExhausted(EntityId),
    /// The evidence record cannot represent another replacement revision.
    #[error("evidence {evidence_id} on node {node} has exhausted its revision space")]
    RevisionSpaceExhausted {
        /// Node which owns the evidence record.
        node: EntityId,
        /// Node-local evidence identity.
        evidence_id: u64,
    },
}
