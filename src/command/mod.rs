use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{Node, NodePayload};

/// One idempotent, revision-checked request to mutate a project graph.
///
/// Clients generate `request_id` once and reuse it if transport fails. The server
/// returns the original [`CommandResult`] for a repeated ID, preventing duplicate
/// graph entities during retries.
///
/// ```
/// use optimist::{
///     command::{CommandRequest, CreateNode, GraphCommand},
///     domain::{Factor, NodePayload},
/// };
///
/// let request = CommandRequest::new(
///     0,
///     GraphCommand::CreateNode(CreateNode {
///         name: "github".to_owned(),
///         title: "GitHub".to_owned(),
///         payload: NodePayload::Factor(Factor {
///             current: None,
///             desired: None,
///             controllable: false,
///             evidence: vec![],
///         }),
///     }),
/// );
/// assert_eq!(request.expected_revision, 0);
/// ```
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CommandRequest {
    /// Client-generated idempotency key retained in the project's command history.
    pub request_id: Uuid,
    /// Project revision on which the client based this command.
    pub expected_revision: u64,
    /// Typed operation to validate and apply.
    pub command: GraphCommand,
}

impl CommandRequest {
    /// Creates a request with a fresh random UUID suitable for one CLI/API attempt.
    pub fn new(expected_revision: u64, command: GraphCommand) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            expected_revision,
            command,
        }
    }
}

/// Mutations accepted by the serialized project command path.
///
/// New variants will preserve the same revision and idempotency envelope.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum GraphCommand {
    /// Allocates an entity ID and inserts a validated node aggregate.
    CreateNode(CreateNode),
}

/// Data required to construct a new structural node.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CreateNode {
    /// Project-unique semantic name used by agents and API callers.
    pub name: String,
    /// Human-facing label shown in graph and detail views.
    pub title: String,
    /// Kind-specific typed fields embedded in the node.
    pub payload: NodePayload,
}

/// Durable result of a committed command, returned identically on retries.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CommandResult {
    /// Request ID whose application produced this result.
    pub request_id: Uuid,
    /// Project revision after the mutation committed.
    pub project_revision: u64,
    /// Typed value created or changed by the command.
    pub outcome: CommandOutcome,
}

/// Typed values returned by graph commands.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum CommandOutcome {
    /// Complete node aggregate created by [`GraphCommand::CreateNode`].
    NodeCreated(Node),
}
