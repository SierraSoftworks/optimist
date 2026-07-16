use serde::{Deserialize, Serialize};

use super::{AnalysisRevisionKey, EdgeId, EntityId, Evidence};

/// Typed evidence references attached to one causal relationship on an impediment path.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RelationshipEvidence {
    /// Canonical causal relationship whose aggregate owns the references.
    pub edge: EdgeId,
    /// Project-defined evidence IDs or citations stored on that relationship.
    pub references: Vec<String>,
}

/// One factor which can reach at least one outcome through the causal graph.
///
/// This is a review candidate, not a causal identification result. The projection
/// reports topology and evidence coverage separately so callers do not mistake an
/// undocumented path for a low probability or combine heterogeneous evidence into
/// a synthetic confidence score.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ImpedimentCandidate {
    /// Factor proposed for impediment review.
    pub factor: EntityId,
    /// Whether the model marks the factor as directly controllable.
    pub controllable: bool,
    /// Outcomes reachable from the factor through causal relationships.
    pub reachable_outcomes: Vec<EntityId>,
    /// Minimum number of causal relationships between the factor and any outcome.
    pub nearest_outcome_distance: usize,
    /// Deterministic union of one canonical shortest path to each reachable outcome.
    pub path_edges: Vec<EdgeId>,
    /// Qualitative evidence records owned directly by the factor.
    pub direct_evidence: Vec<Evidence>,
    /// Typed evidence references found on causal path relationships.
    pub relationship_evidence: Vec<RelationshipEvidence>,
    /// Path relationships with no typed evidence references.
    pub unsupported_path_edges: Vec<EdgeId>,
}

/// Deterministic impediment-review projection for one immutable graph snapshot.
///
/// `topology_candidates` is ordered by outcome reach, shortest distance, then factor
/// ID. `evidence_priority` is a separate lexicographic review order: direct evidence
/// count, relationship evidence-reference count, topology order, then factor ID.
/// These are transparent review heuristics, not statistical confidence or causal
/// effect estimates.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ImpedimentAnalysis {
    /// Revisions proving which graph/documents produced the projection.
    pub revision: AnalysisRevisionKey,
    /// Factors ordered using topology only.
    pub topology_candidates: Vec<ImpedimentCandidate>,
    /// Factor IDs ordered using explicit evidence coverage before topology.
    pub evidence_priority: Vec<EntityId>,
}
