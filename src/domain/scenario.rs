use serde::{Deserialize, Serialize};

use super::{EntityId, MonteCarloConfig, ScenarioError, ScenarioId, Unit, scenario_validation};

/// Preference direction for one outcome objective.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UtilityDirection {
    /// Prefer larger outcome values.
    Maximize,
    /// Prefer smaller outcome values.
    Minimize,
}

/// One weighted outcome referenced by a scenario.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ScenarioObjective {
    /// Project-local entity ID which must resolve to an outcome.
    pub outcome_id: EntityId,
    /// Direction in which the outcome contributes utility.
    pub direction: UtilityDirection,
    /// Finite positive relative importance within this scenario.
    pub importance: f64,
}

/// A finite positive limit for one project-defined resource dimension.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ScenarioBudget {
    /// Runtime unit identifying the constrained resource dimension.
    pub unit: Unit,
    /// Finite positive maximum available amount.
    pub amount: f64,
}

/// Explicit conversion used only when a scalar comparison is requested.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ScalarPreference {
    /// Runtime unit whose values may be converted into scalar utility.
    pub unit: Unit,
    /// Finite positive utility cost assigned to one unit.
    pub utility_per_unit: f64,
}

/// Input used to create a validated scenario document.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ScenarioDraft {
    /// Project-unique human and agent-facing scenario name.
    pub name: String,
    /// Human-facing display title.
    pub title: String,
    /// Markdown explanation of assumptions and decision context.
    #[serde(default)]
    pub rationale: String,
    /// Outcomes which define success for the scenario.
    pub objectives: Vec<ScenarioObjective>,
    /// Number of discrete planning periods; zero is invalid.
    pub planning_horizon: u64,
    /// Independent resource limits retained as a vector for Pareto analysis.
    #[serde(default)]
    pub budgets: Vec<ScenarioBudget>,
    /// Project-local entity IDs which must resolve to interventions.
    #[serde(default)]
    pub candidate_interventions: Vec<EntityId>,
    /// Deterministic probability-sampling controls for eventual analysis.
    pub monte_carlo: MonteCarloConfig,
    /// Optional scalar conversions, present only when explicitly supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scalar_preferences: Option<Vec<ScalarPreference>>,
}

/// Revisioned scenario document stored outside the causal graph.
///
/// Call [`Scenario::new`] before project reference validation so aggregate-local
/// failures are reported without graph access. The project command path then proves
/// that objectives are outcomes and candidates are interventions.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Scenario {
    /// Project-local identity from the scenario namespace.
    pub id: ScenarioId,
    /// Document revision used for update and delete conflict detection.
    pub revision: u64,
    /// Validated document fields supplied by the caller.
    #[serde(flatten)]
    pub draft: ScenarioDraft,
}

impl Scenario {
    /// Constructs revision zero after validating aggregate-local invariants.
    ///
    /// Project validation separately resolves objective and candidate references.
    pub fn new(id: ScenarioId, draft: ScenarioDraft) -> Result<Self, ScenarioError> {
        draft.validate()?;
        Ok(Self {
            id,
            revision: 0,
            draft,
        })
    }
}

impl ScenarioDraft {
    /// Validates fields which do not require project graph access.
    pub fn validate(&self) -> Result<(), ScenarioError> {
        scenario_validation::validate(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> ScenarioDraft {
        ScenarioDraft {
            name: "delivery".to_owned(),
            title: "Delivery reliability".to_owned(),
            rationale: "Prioritize sustained delivery.".to_owned(),
            objectives: vec![ScenarioObjective {
                outcome_id: EntityId::new(1),
                direction: UtilityDirection::Maximize,
                importance: 1.0,
            }],
            planning_horizon: 12,
            budgets: vec![ScenarioBudget {
                unit: Unit::base("usd").unwrap(),
                amount: 10_000.0,
            }],
            candidate_interventions: vec![EntityId::new(2)],
            monte_carlo: MonteCarloConfig::new(42, 100, 1_000, 0.01, 0.01).unwrap(),
            scalar_preferences: None,
        }
    }

    #[test]
    fn creates_revisioned_document_with_independent_compact_id() {
        let scenario = Scenario::new(ScenarioId::new(0), draft()).unwrap();
        assert_eq!(scenario.id.to_string(), "A");
        assert_eq!(scenario.revision, 0);
        assert!(
            !serde_json::to_value(scenario)
                .unwrap()
                .as_object()
                .unwrap()
                .contains_key("scalar_preferences")
        );
    }

    #[test]
    fn rejects_duplicate_references_and_nonpositive_values() {
        let mut value = draft();
        value.objectives.push(value.objectives[0].clone());
        assert_eq!(value.validate(), Err(ScenarioError::DuplicateObjective));

        let mut value = draft();
        value.budgets[0].amount = f64::NAN;
        assert_eq!(value.validate(), Err(ScenarioError::InvalidBudget));

        let mut value = draft();
        value.planning_horizon = 0;
        assert_eq!(value.validate(), Err(ScenarioError::ZeroPlanningHorizon));
    }
}
