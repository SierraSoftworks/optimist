use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{EdgeId, EntityId, ProjectId, ScenarioId};

/// Immutable document revisions which completely identify one analysis input snapshot.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AnalysisRevisionKey {
    /// Isolated project whose graph and documents were projected.
    pub project: ProjectId,
    /// Project graph revision after the latest committed command.
    pub graph_revision: u64,
    /// Selected scenario and its independent revision, when analysis is scenario-scoped.
    pub scenario: Option<(ScenarioId, u64)>,
    /// Residual dependence document revision, or `None` when independence is explicit.
    pub dependence_revision: Option<u64>,
}

/// Explicit computational bounds for deterministic structural analysis.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AnalysisLimits {
    /// Maximum edges in one elementary cycle; must be greater than zero.
    pub maximum_cycle_length: usize,
    /// Maximum elementary cycles returned before enumeration stops; must be positive.
    pub maximum_cycles: usize,
}

impl Default for AnalysisLimits {
    fn default() -> Self {
        Self {
            maximum_cycle_length: 8,
            maximum_cycles: 1_000,
        }
    }
}

impl AnalysisLimits {
    /// Constructs positive bounds for elementary-cycle enumeration.
    pub fn new(maximum_cycle_length: usize, maximum_cycles: usize) -> Result<Self, AnalysisError> {
        if maximum_cycle_length == 0 || maximum_cycles == 0 {
            return Err(AnalysisError::InvalidLimits);
        }
        Ok(Self {
            maximum_cycle_length,
            maximum_cycles,
        })
    }
}

/// Failures which prevent exact structural projection from starting.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AnalysisError {
    /// Cycle length and result-count bounds must both be greater than zero.
    #[error("analysis cycle limits must be greater than zero")]
    InvalidLimits,
    /// A causal edge references an entity absent from the immutable node snapshot.
    #[error("causal edge {edge} references missing node {node}")]
    MissingNode {
        /// Invalid causal edge identity.
        edge: EdgeId,
        /// Missing source or destination node.
        node: EntityId,
    },
}

/// One maximal strongly connected component in the directed causal graph.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StronglyConnectedComponent {
    /// Component members in canonical entity-ID order.
    pub nodes: Vec<EntityId>,
    /// Causal edges whose endpoints both belong to this component.
    pub edges: Vec<EdgeId>,
    /// Whether the component contains feedback: multiple nodes or a self-loop.
    pub is_feedback: bool,
}

/// One canonical directed elementary cycle with no repeated internal node.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ElementaryCycle {
    /// Node order rotated so the smallest entity ID is first.
    pub nodes: Vec<EntityId>,
    /// Directed edge identities in traversal order, including the closing edge.
    pub edges: Vec<EdgeId>,
}

/// Exact deterministic topology derived from one immutable project snapshot.
///
/// Only `contributes`, `changes`, and `blocks` are causal. This result makes no
/// statistical claims about edge strength, feedback stability, or intervention
/// impact; those require a separately documented posterior propagation model.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StructuralAnalysis {
    /// Revisions proving which graph/documents produced this result.
    pub revision: AnalysisRevisionKey,
    /// Strongly connected components ordered by their smallest member ID.
    pub components: Vec<StronglyConnectedComponent>,
    /// Canonical bounded elementary cycles in lexical node/edge order.
    pub cycles: Vec<ElementaryCycle>,
    /// Whether cycle enumeration stopped at the configured count bound.
    pub cycles_truncated: bool,
    /// Limits applied while computing this result.
    pub limits: AnalysisLimits,
}
