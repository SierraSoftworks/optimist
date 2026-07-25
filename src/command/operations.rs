use serde::{Deserialize, Serialize};

use crate::domain::{
    EdgeId, EdgePayload, EntityId, EstimateAddress, EstimateSlot, MeasurementCalibration,
    NewObservation, NodePayload, ProjectDependenceModel, QuantityDefinition, ScenarioDraft,
    ScenarioId, SquiggleEstimateDefinition, StateRelation,
};

use super::EffectProfileInput;

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

/// Revision-checked configuration or replacement of native factor or outcome state type.
///
/// Existing Squiggle estimates are reevaluated against the proposed canonical unit and support.
/// Canonical dimension changes are rejected while an incident causal response uses the old unit.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetNodeQuantityState {
    /// Factor or outcome node whose state quantity is being configured or edited.
    pub node: EntityId,
    /// Node revision observed before preparing the quantity definition.
    pub expected_revision: u64,
    /// Canonical native unit, support, and operational definition.
    pub quantity: QuantityDefinition,
}

/// Revision-checked replacement of one state's node equation.
///
/// The equation is compiled against the graph before it is stored, so a name it
/// cannot bind or arithmetic whose units do not work out is rejected here rather
/// than surfacing later as a failed projection. Passing `None` restores
/// proportional composition from the relationship responses.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetStateRelation {
    /// Factor, outcome, or metric whose value the equation computes.
    pub node: EntityId,
    /// Node revision observed before preparing the equation.
    pub expected_revision: u64,
    /// Complete equation, or `None` to restore proportional composition.
    pub relation: Option<StateRelation>,
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

/// Revision-checked replacement of one causal relationship's stated reasoning.
///
/// Strength is a dimensionless response estimate edited through the estimate
/// commands; this replaces the prose and citations that justify it, which is what
/// a reviewer reads before trusting the number.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateCausalEffect {
    /// Canonical identity of the `contributes` or `changes` relationship.
    pub edge: EdgeId,
    /// Edge revision observed before preparing the update.
    pub expected_revision: u64,
    /// Markdown explanation of the mechanism, boundaries, and assumptions.
    pub mechanism: String,
    /// Evidence references supporting this relationship.
    #[serde(default)]
    pub evidence: Vec<String>,
}

/// Revision-checked replacement of one intervention effect's temporal shape.
///
/// The profile owns every estimate it needs, so it is authored as one document
/// rather than assembled from individually addressed slots. Passing `None`
/// restores the permanent step applied when no profile is configured.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetEffectProfile {
    /// Canonical identity of the `changes` relationship being shaped.
    pub edge: EdgeId,
    /// Edge revision observed before preparing the profile.
    pub expected_revision: u64,
    /// Complete replacement, or `None` to restore a permanent effect.
    pub profile: Option<Box<EffectProfileInput>>,
}

/// Data required to create or replace an estimate from backend-evaluated Squiggle source.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetSquiggleEstimate {
    /// Stable project/owner/estimate identity; nested components are unsupported.
    pub address: EstimateAddress,
    /// Semantic owner field whose support and unit are enforced server-side.
    pub slot: EstimateSlot,
    /// Reviewable source, target unit, and deterministic evaluation controls.
    pub definition: SquiggleEstimateDefinition,
    /// Evidence or elicitation records supporting this estimate revision.
    #[serde(default)]
    pub provenance: Vec<String>,
    /// Distinct epistemic, process, and measurement uncertainty assumptions.
    #[serde(default)]
    pub uncertainty: crate::domain::EstimateUncertainty,
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
