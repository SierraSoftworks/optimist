use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{
    Edge, Evidence, FormulaDefinition, Node, Observation, PrimitiveEstimate,
    ProjectDependenceModel, Scenario,
};

mod change_set;
mod classification;
mod metadata_operations;
mod operations;

pub use change_set::{ChangeSet, ChangeSetReplay, ChangeStreamMessage};
pub use metadata_operations::*;
pub use operations::*;

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
    /// Replaces one node's title, Markdown description, and metadata map.
    UpdateNodeMetadata(UpdateNodeMetadata),
    /// Appends qualitative evidence to a factor or outcome.
    CreateEvidence(CreateEvidence),
    /// Replaces one evidence record under its aggregate-local revision guard.
    UpdateEvidence(UpdateEvidence),
    /// Removes one evidence record under its aggregate-local revision guard.
    DeleteEvidence(DeleteEvidence),
    /// Validates stored endpoint kinds and inserts one canonical structural edge.
    CreateEdge(CreateEdge),
    /// Removes one structural edge while retaining both endpoint nodes.
    DeleteEdge(DeleteEdge),
    /// Replaces one edge's Markdown description and metadata map.
    UpdateEdgeMetadata(UpdateEdgeMetadata),
    /// Appends a validated immutable reading to a `measures` edge.
    AppendObservation(AppendObservation),
    /// Appends a correction which supersedes one existing observation.
    CorrectObservation(CorrectObservation),
    /// Replaces or removes one measurement relationship's reading-to-state calibration.
    SetMeasurementCalibration(SetMeasurementCalibration),
    /// Creates or replaces one primitive estimate in a typed owner field.
    SetEstimate(SetEstimate),
    /// Removes one optional or named-cost estimate from its owner.
    RemoveEstimate(RemoveEstimate),
    /// Creates or replaces one nested Fermi component formula.
    SetFormula(SetFormula),
    /// Removes one unreferenced leaf Fermi component formula.
    RemoveFormula(RemoveFormula),
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
    /// Complete node aggregate updated by [`GraphCommand::UpdateNodeMetadata`].
    NodeMetadataUpdated(Node),
    /// New evidence record and complete updated owning node.
    EvidenceCreated {
        /// Complete updated factor or outcome after persistence.
        node: Node,
        /// Node-local evidence record allocated by the command.
        evidence: Evidence,
    },
    /// Replaced evidence record and complete updated owning node.
    EvidenceUpdated {
        /// Complete updated factor or outcome after persistence.
        node: Node,
        /// Evidence record after its revision advanced.
        evidence: Evidence,
    },
    /// Removed evidence record and complete updated owning node.
    EvidenceDeleted {
        /// Complete updated factor or outcome after persistence.
        node: Node,
        /// Evidence record removed by the command.
        evidence: Evidence,
    },
    /// Complete canonical edge created by [`GraphCommand::CreateEdge`].
    EdgeCreated(Edge),
    /// Complete canonical edge removed by [`GraphCommand::DeleteEdge`].
    EdgeDeleted(Edge),
    /// Complete edge aggregate updated by [`GraphCommand::UpdateEdgeMetadata`].
    EdgeMetadataUpdated(Edge),
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
    /// Complete measurement edge after its calibration was replaced or removed.
    MeasurementCalibrationSet(Edge),
    /// Primitive estimate created or revisioned by [`GraphCommand::SetEstimate`].
    EstimateSet(PrimitiveEstimate),
    /// Primitive estimate removed by [`GraphCommand::RemoveEstimate`].
    EstimateRemoved(PrimitiveEstimate),
    /// Fermi component created or replaced by [`GraphCommand::SetFormula`].
    FormulaSet(FormulaDefinition),
    /// Fermi component removed by [`GraphCommand::RemoveFormula`].
    FormulaRemoved(FormulaDefinition),
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
