use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{CommandOutcome, GraphCommand};

/// One committed, ordered project mutation suitable for deterministic replay.
///
/// The event is appended only after command application succeeds. An idempotent
/// retry with the same request ID returns its original result and does not append
/// another event. Current storage is process-local; durable backends must persist
/// this exact event before acknowledging multi-item mutations.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChangeSet {
    /// Client-generated idempotency key from the original command request.
    pub request_id: Uuid,
    /// Project revision on which the client based the command.
    pub base_revision: u64,
    /// Project revision assigned after the command committed.
    pub project_revision: u64,
    /// Independent graph revision after commit; document-only commands do not advance it.
    pub graph_revision: u64,
    /// Typed command which produced this event.
    pub command: GraphCommand,
    /// Complete committed value returned to the client.
    pub outcome: CommandOutcome,
}

/// Deterministic committed event replay after one observed project revision.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChangeSetReplay {
    /// Exclusive lower project revision supplied by the caller.
    pub after_revision: u64,
    /// Current project revision when the replay snapshot was created.
    pub current_revision: u64,
    /// Committed changes in ascending project-revision order.
    pub changes: Vec<ChangeSet>,
}
