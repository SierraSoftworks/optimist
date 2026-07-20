use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{CommandResult, GraphCommand};

/// Maximum number of commands accepted in one atomic forward or compensation batch.
pub const MAX_COMMAND_BATCH_SIZE: usize = 100;

/// One bounded sequence of commands committed atomically against a project revision.
///
/// Each command receives a deterministic child request ID derived from
/// [`CommandBatchRequest::request_id`] and its zero-based position. Retrying the
/// same batch therefore returns the original child results without duplicating
/// mutations or replay events.
///
/// ```
/// use optimist::command::CommandBatchRequest;
/// let request: CommandBatchRequest = serde_json::from_str(
///     r#"{"request_id":"00000000-0000-4000-8000-000000000001","expected_revision":4,"commands":[]}"#,
/// )?;
/// assert_eq!(request.expected_revision, 4);
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CommandBatchRequest {
    /// Client-generated idempotency key for the complete batch.
    pub request_id: Uuid,
    /// Project revision on which every command and its ordering were prepared.
    pub expected_revision: u64,
    /// Typed commands applied in order or not at all.
    pub commands: Vec<GraphCommand>,
}

/// A reviewed compensation plan for one previously committed command batch.
///
/// Compensation is a new forward mutation, not history erasure. Callers provide
/// commands valid against the current project so immutable facts can be corrected
/// according to domain semantics instead of being silently deleted.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CompensatingUndoRequest {
    /// Client-generated idempotency key for this compensation attempt.
    pub request_id: Uuid,
    /// Current project revision observed while preparing the compensation plan.
    pub expected_revision: u64,
    /// Reviewed commands which compensate the original batch in order.
    pub commands: Vec<GraphCommand>,
}

/// Durable result of an atomic command batch or later compensation batch.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CommandBatchResult {
    /// Idempotency key of this forward or compensation batch.
    pub request_id: Uuid,
    /// Project revision observed before the first command committed.
    pub base_revision: u64,
    /// Project revision after the final command committed.
    pub project_revision: u64,
    /// Original batch compensated by this result, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compensates: Option<Uuid>,
    /// Child command results in the same order as the submitted commands.
    pub results: Vec<CommandResult>,
}

pub(crate) fn child_request_id(batch: Uuid, index: usize) -> Uuid {
    Uuid::new_v5(&batch, &(index as u64).to_be_bytes())
}
