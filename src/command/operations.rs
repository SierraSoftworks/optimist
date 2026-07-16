use serde::{Deserialize, Serialize};

use crate::domain::{
    Distribution, EdgeId, EdgePayload, EntityId, EstimateAddress, EstimateSlot, Formula,
    MeasurementCalibration, NewObservation, NodePayload, ProjectDependenceModel, ScenarioDraft,
    ScenarioId,
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

/// Data required to append qualitative evidence to a factor or outcome.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CreateEvidence {
    /// Project-local factor or outcome which owns the evidence record.
    pub node: EntityId,
    /// Concise Markdown-compatible observation or symptom description.
    pub summary: String,
    /// Optional citation, URL, system, or person which supplied the evidence.
    pub source: Option<String>,
}

/// Revision-checked replacement of one node-owned evidence record.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct UpdateEvidence {
    /// Project-local factor or outcome which owns the evidence record.
    pub node: EntityId,
    /// Aggregate-local evidence identifier.
    pub evidence_id: u64,
    /// Evidence revision observed before preparing the replacement.
    pub expected_revision: u64,
    /// Complete replacement summary.
    pub summary: String,
    /// Complete replacement source, or `None` when no source is known.
    pub source: Option<String>,
}

/// Revision-checked identity of one node-owned evidence record to remove.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DeleteEvidence {
    /// Project-local factor or outcome which owns the evidence record.
    pub node: EntityId,
    /// Aggregate-local evidence identifier.
    pub evidence_id: u64,
    /// Evidence revision observed before deletion.
    pub expected_revision: u64,
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

/// Revision-checked replacement of one measurement relationship's state calibration.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SetMeasurementCalibration {
    /// Canonical identity of the `measures` relationship being calibrated.
    pub edge: EdgeId,
    /// Edge revision observed before preparing the calibration.
    pub expected_revision: u64,
    /// Complete replacement, or `None` to return the relationship to descriptive polarity only.
    pub calibration: Option<MeasurementCalibration>,
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

/// Revision-checked creation or replacement of one nested Fermi component.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SetFormula {
    /// Nested component address under an existing primitive estimate root.
    pub address: EstimateAddress,
    /// Formula source validated against all primitive and component definitions.
    pub formula: Formula,
    /// Formula document revision observed by the caller.
    pub expected_revision: u64,
    /// Evidence or elicitation context for this decomposition.
    #[serde(default)]
    pub provenance: Vec<String>,
}

/// Revision-checked removal of one unreferenced leaf Fermi component.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RemoveFormula {
    /// Existing nested component address to remove.
    pub address: EstimateAddress,
    /// Formula document revision observed by the caller.
    pub expected_revision: u64,
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
