use serde::{Deserialize, Serialize};

use crate::domain::{
    Distribution, EdgeId, EdgePayload, EntityId, EstimateAddress, EstimateSlot, NewObservation,
    NodePayload, ProjectDependenceModel, ScenarioDraft, ScenarioId,
};

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

/// Data required to create or replace a primitive embedded estimate.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SetEstimate {
    /// Stable project/owner/estimate identity; nested components are unsupported.
    pub address: EstimateAddress,
    /// Semantic owner field whose type validates distribution support.
    pub slot: EstimateSlot,
    /// Primitive distribution to validate against the selected slot dimension.
    pub distribution: Distribution,
    /// Evidence or elicitation records supporting this estimate revision.
    #[serde(default)]
    pub provenance: Vec<String>,
}

/// Identity of an optional or named-cost primitive estimate to remove.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RemoveEstimate {
    /// Existing root estimate address; nested components are unsupported.
    pub address: EstimateAddress,
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
