use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{
    Edge, EdgeId, EdgePayload, EntityId, NewObservation, Node, NodePayload, Observation,
    ProjectDependenceModel, Scenario, ScenarioDraft, ScenarioId,
};

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
    /// Removes a node after the repository verifies that it has no incident edges.
    DeleteNode(DeleteNode),
    /// Validates stored endpoint kinds and inserts one canonical structural edge.
    CreateEdge(CreateEdge),
    /// Removes one structural edge while retaining both endpoint nodes.
    DeleteEdge(DeleteEdge),
    /// Appends a validated immutable reading to a `measures` edge.
    AppendObservation(AppendObservation),
    /// Appends a correction which supersedes one existing observation.
    CorrectObservation(CorrectObservation),
    /// Allocates an independent project-local ID and stores a scenario document.
    CreateScenario(CreateScenario),
    /// Replaces a scenario document under its own revision guard.
    UpdateScenario(UpdateScenario),
    /// Removes a scenario document under its own revision guard.
    DeleteScenario(DeleteScenario),
    /// Creates or replaces the project's Gaussian residual dependence document.
    SetProjectDependence(SetProjectDependence),
    /// Removes the project's Gaussian residual dependence document.
    RemoveProjectDependence(RemoveProjectDependence),
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

/// Identity of a structural node to remove from the project graph.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DeleteNode {
    /// Project-local ID of the node, which must have no incident edges.
    pub id: EntityId,
}

/// Data required to construct a structural relationship between existing nodes.
///
/// Endpoint kinds are intentionally absent: the project executor derives them from
/// stored nodes before calling `Edge::new`, preventing clients from forging type
/// declarations to bypass the endpoint matrix.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CreateEdge {
    /// Project-local outbound entity ID.
    pub source: EntityId,
    /// Project-local inbound entity ID.
    pub destination: EntityId,
    /// Kind-specific fields and embedded values for the relationship.
    pub payload: EdgePayload,
}

/// Identity of a structural edge to remove from the project graph.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DeleteEdge {
    /// Canonical edge identity derived from its endpoints and kind.
    pub id: EdgeId,
}

/// Data required to append a reading to an existing measurement edge.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AppendObservation {
    /// Canonical ID of the `measures` edge which owns the observation series.
    pub edge: EdgeId,
    /// Unidentified reading; the edge allocates its local observation ID.
    pub observation: NewObservation,
}

/// Data required to append an immutable correction to a measurement series.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CorrectObservation {
    /// Canonical ID of the `measures` edge owning the original observation.
    pub edge: EdgeId,
    /// Edge-local ID of the unsuperseded observation being corrected.
    pub observation_id: u64,
    /// Finite corrected value; other provenance fields are copied from the original.
    pub value: f64,
}

/// Data required to create a scenario outside the causal graph.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CreateScenario {
    /// Validated scenario fields and graph references awaiting project resolution.
    pub scenario: ScenarioDraft,
}

/// Revision-checked replacement for an existing scenario document.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct UpdateScenario {
    /// Project-local scenario document ID.
    pub id: ScenarioId,
    /// Scenario revision on which the replacement was based.
    pub expected_revision: u64,
    /// Complete replacement fields; partial patch semantics are intentionally absent.
    pub scenario: ScenarioDraft,
}

/// Revision-checked identity of a scenario document to remove.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DeleteScenario {
    /// Project-local scenario document ID.
    pub id: ScenarioId,
    /// Scenario revision observed before deletion.
    pub expected_revision: u64,
}

/// Complete revision-checked replacement for project residual dependence.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SetProjectDependence {
    /// Complete model whose revision must match the stored document on replacement.
    pub model: ProjectDependenceModel,
}

/// Revision-checked request to remove project residual dependence.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RemoveProjectDependence {
    /// Dependence document revision observed by the caller.
    pub expected_revision: u64,
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
    /// Complete node aggregate removed by [`GraphCommand::DeleteNode`].
    NodeDeleted(Node),
    /// Complete canonical edge created by [`GraphCommand::CreateEdge`].
    EdgeCreated(Edge),
    /// Complete canonical edge removed by [`GraphCommand::DeleteEdge`].
    EdgeDeleted(Edge),
    /// New immutable reading and updated owning measurement edge.
    ObservationAppended {
        /// Complete updated edge aggregate after persistence.
        edge: Edge,
        /// Observation allocated and appended by the measurement aggregate.
        observation: Observation,
    },
    /// New correction record and updated owning measurement edge.
    ObservationCorrected {
        /// Complete updated edge aggregate after persistence.
        edge: Edge,
        /// Immutable correction whose `supersedes` points at its predecessor.
        observation: Observation,
    },
    /// Complete scenario document created by [`GraphCommand::CreateScenario`].
    ScenarioCreated(Scenario),
    /// Complete replacement stored by [`GraphCommand::UpdateScenario`].
    ScenarioUpdated(Scenario),
    /// Complete document removed by [`GraphCommand::DeleteScenario`].
    ScenarioDeleted(Scenario),
    /// Complete dependence document stored by [`GraphCommand::SetProjectDependence`].
    ProjectDependenceSet(ProjectDependenceModel),
    /// Complete dependence document removed by [`GraphCommand::RemoveProjectDependence`].
    ProjectDependenceRemoved(ProjectDependenceModel),
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;

    use crate::domain::{EdgeId, EdgeKind, EntityId};

    use super::{CommandRequest, DeleteEdge, DeleteNode, GraphCommand};

    #[test]
    fn delete_commands_have_stable_tagged_json() {
        let node = CommandRequest {
            request_id: Uuid::nil(),
            expected_revision: 7,
            command: GraphCommand::DeleteNode(DeleteNode {
                id: EntityId::new(0),
            }),
        };
        assert_eq!(
            serde_json::to_value(&node).unwrap(),
            json!({
                "request_id": "00000000-0000-0000-0000-000000000000",
                "expected_revision": 7,
                "command": {"type": "delete_node", "payload": {"id": "A"}}
            })
        );
        assert_eq!(
            serde_json::from_value::<CommandRequest>(serde_json::to_value(&node).unwrap()).unwrap(),
            node
        );

        let edge = GraphCommand::DeleteEdge(DeleteEdge {
            id: EdgeId {
                source: EntityId::new(1),
                kind: EdgeKind::Requires,
                destination: EntityId::new(0),
            },
        });
        assert_eq!(
            serde_json::to_value(&edge).unwrap(),
            json!({
                "type": "delete_edge",
                "payload": {
                    "id": {"source": "B", "kind": "requires", "destination": "A"}
                }
            })
        );
        assert_eq!(
            serde_json::from_value::<GraphCommand>(serde_json::to_value(&edge).unwrap()).unwrap(),
            edge
        );
    }
}
