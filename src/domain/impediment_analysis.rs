use serde::{Deserialize, Serialize};

use super::{AnalysisRevisionKey, Distribution, EntityId};

/// One factor requirement which can block an intervention execution step.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct InterventionRequirement {
    /// Intervention which owns the requirement.
    pub dependent: EntityId,
    /// Factor whose state must satisfy the requirement.
    pub prerequisite: EntityId,
    /// Whether an unsatisfied requirement precludes execution.
    pub hard: bool,
    /// Optional normalized factor-state threshold.
    pub satisfaction_threshold: Option<f64>,
}

/// One intervention in prerequisite-first execution order.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct InterventionExecutionStep {
    /// Intervention executed at this step.
    pub intervention: EntityId,
    /// Uncertain completion duration, or immediate when omitted.
    pub duration: Option<Distribution>,
    /// Uncertain success probability, or certain success when omitted.
    pub probability_of_success: Option<Distribution>,
}

/// Intervention readiness projection derived from execution dependencies.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ImpedimentCandidate {
    /// Intervention being considered.
    pub intervention: EntityId,
    /// Recursive prerequisite interventions followed by this intervention.
    pub execution_steps: Vec<InterventionExecutionStep>,
    /// Factor requirements which cannot be completed as intervention steps.
    pub blocking_requirements: Vec<InterventionRequirement>,
    /// Interventions connected by declared synergy but outside this execution plan.
    pub synergies: Vec<EntityId>,
    /// Interventions declared incompatible with this execution plan.
    pub conflicts: Vec<EntityId>,
    /// Sum of expected prerequisite and candidate durations.
    pub expected_duration: f64,
    /// Product of prerequisite and candidate mean success probabilities.
    pub expected_success_probability: f64,
}

/// Deterministic execution-readiness projection for all project interventions.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ImpedimentAnalysis {
    /// Revisions proving which graph produced the projection.
    pub revision: AnalysisRevisionKey,
    /// Interventions ordered by blockers, success, duration, then identity.
    pub candidates: Vec<ImpedimentCandidate>,
}
