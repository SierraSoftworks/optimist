use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{CommandOutcome, GraphCommand};
use crate::project::ProjectArchive;

/// One committed, ordered project mutation suitable for deterministic replay.
///
/// The event is appended only after command application succeeds. An idempotent
/// retry with the same request ID returns its original result and does not append
/// another event. Persistent servers publish this event atomically with the project
/// snapshot before acknowledging the mutation.
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
    /// Complete replacement state when [`ChangeSetReplay::after_revision`] predates retained history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<ChangeSnapshot>,
}

/// Canonical project replacement supplied when incremental replay cannot bridge a history gap.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeSnapshot {
    /// Project revision represented by [`ChangeSnapshot::archive`].
    pub revision: u64,
    /// Complete validated project state to install before resuming live changes.
    pub archive: ProjectArchive,
}

/// Server-to-client message on a project ChangeSet WebSocket stream.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ChangeStreamMessage {
    /// Complete project replacement required before subsequent live changes are applied.
    Snapshot(Box<ChangeSnapshot>),
    /// One replayed or newly committed change in project-revision order.
    Change(Box<ChangeSet>),
    /// Replay is complete and live events begin after this revision.
    CaughtUp {
        /// Latest project revision included in replay.
        revision: u64,
    },
    /// The bounded live receiver lagged and the client must reconnect from this cursor.
    ReplayRequired {
        /// Last project revision delivered successfully to this client.
        after_revision: u64,
    },
}
