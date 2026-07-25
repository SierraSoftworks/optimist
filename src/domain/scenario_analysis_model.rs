use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    AnalysisRevisionKey, EntityId, InterventionRequirement, MonteCarloDiagnostics,
    MonteCarloEstimate, ScenarioId, UtilityDirection,
};

/// Statistical state and improvement summary at one planning period.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ObjectiveTrajectoryPoint {
    /// Zero-based period, where zero is the sampled baseline before intervention arrival.
    pub period: u64,
    /// Objective state at this period.
    pub state: MonteCarloEstimate,
    /// Direction-oriented movement from the sampled baseline at this period.
    pub improvement: MonteCarloEstimate,
}

/// Statistical summary of one scenario objective under one intervention.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ObjectiveProjection {
    /// Outcome whose normalized state was propagated.
    pub outcome: EntityId,
    /// Scenario preference used to orient improvement.
    pub direction: UtilityDirection,
    /// Relative importance retained from the scenario document.
    pub importance: f64,
    /// Whether this candidate has a directed causal path to the objective.
    ///
    /// An unreachable objective still reports its sampled baseline and zero movement;
    /// callers should not interpret that zero as evidence of ineffectiveness.
    pub reachable: bool,
    /// Sampled baseline normalized state.
    pub baseline: MonteCarloEstimate,
    /// Normalized state at the end of the planning horizon.
    pub final_state: MonteCarloEstimate,
    /// Direction-oriented improvement, positive when the outcome improves.
    pub improvement: MonteCarloEstimate,
    /// Per-period state and improvement from baseline through the planning horizon.
    pub trajectory: Vec<ObjectiveTrajectoryPoint>,
}

/// Finite-horizon posterior projection for one candidate intervention.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct InterventionProjection {
    /// Candidate intervention whose prerequisite plan is evaluated independently of other candidates.
    pub intervention: EntityId,
    /// Prerequisite interventions in execution order, excluding the candidate.
    pub prerequisites: Vec<EntityId>,
    /// Factor requirements which can block the execution plan.
    pub blocking_requirements: Vec<InterventionRequirement>,
    /// Declared synergies outside the required execution plan.
    pub synergies: Vec<EntityId>,
    /// Declared conflicts outside the required execution plan.
    pub conflicts: Vec<EntityId>,
    /// Total prerequisite-plus-candidate execution duration.
    pub execution_duration: MonteCarloEstimate,
    /// Bernoulli summary for every required intervention succeeding.
    pub execution_success: MonteCarloEstimate,
    /// Per-objective projections in scenario document order.
    pub objectives: Vec<ObjectiveProjection>,
    /// Sample covariance of direction-oriented improvements in objective order.
    ///
    /// Entry $(i,j)$ estimates $\operatorname{Cov}(\Delta_i,\Delta_j)$ with
    /// Bessel's correction and is absent below two valid joint draws. This retains
    /// dependence induced by shared sampled graph paths without collapsing distinct
    /// objectives into scalar utility.
    pub improvement_covariance: Vec<Vec<Option<f64>>>,
    /// Number of state-period updates clamped to their declared support boundary.
    ///
    /// Clamping keeps each recurrence inside its state support but can hide saturation or
    /// unstable feedback. A nonzero count is therefore retained as a model
    /// diagnostic rather than treated as an invalid Monte Carlo draw. The count is
    /// accumulated across every valid draw, planning period, and relevant state.
    pub clamped_state_updates: u64,
    /// Number of proportional responses dropped for want of a ratio scale.
    ///
    /// A source whose sampled baseline is zero has no fractional movement, so a
    /// response reading from it is skipped rather than propagating an infinity.
    /// A nonzero count means part of the model was silently inert for those draws,
    /// which is a modelling problem to fix rather than an invalid draw to discard.
    pub undefined_responses: u64,
    /// Reproducibility and convergence information for this candidate run.
    pub diagnostics: MonteCarloDiagnostics,
}

/// Scenario analysis result tied to one immutable project snapshot.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ScenarioAnalysis {
    /// Revisions proving which graph and documents produced this result.
    pub revision: AnalysisRevisionKey,
    /// Number of synchronous planning periods propagated.
    pub planning_horizon: u64,
    /// Independently evaluated candidate execution plans in scenario document order.
    pub candidates: Vec<InterventionProjection>,
}

/// Failures which prevent finite-horizon scenario propagation.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ScenarioAnalysisError {
    /// The revision key must identify the scenario being evaluated.
    #[error("analysis revision does not identify scenario {0}")]
    RevisionMismatch(ScenarioId),
    /// A scenario objective has no normalized baseline estimate.
    #[error("scenario objective {0} has no current normalized-state estimate")]
    MissingObjectiveBaseline(EntityId),
    /// A causal factor participating in propagation has no normalized baseline.
    #[error("causal factor {0} has no current normalized-state estimate")]
    MissingFactorBaseline(EntityId),
    /// A causal metric participating in propagation has no native-unit baseline.
    #[error("causal metric {0} has no current native-unit estimate")]
    MissingMetricBaseline(EntityId),
    /// A scenario reference is absent or has the wrong node kind.
    #[error("scenario reference {0} is absent or has the wrong node kind")]
    InvalidReference(EntityId),
    /// A causal edge references a state node without a baseline.
    #[error("causal edge references missing state node {0}")]
    MissingCausalNode(EntityId),
    /// A sampled primitive or propagated state was not finite.
    #[error("scenario propagation produced a non-finite value")]
    NonFiniteResult,
    /// A duration or lag sampler produced NaN or infinity.
    #[error("scenario propagation sampled a non-finite primitive duration or lag")]
    NonFinitePrimitive,
    /// Dynamic scenario sampling does not yet apply project copula dependence.
    #[error("scenario analysis does not yet support non-empty project dependence models")]
    UnsupportedDependence,
    /// Intervention `requires` relationships contain a dependency cycle.
    #[error("intervention dependency cycle includes {0}")]
    InterventionDependencyCycle(EntityId),
}
