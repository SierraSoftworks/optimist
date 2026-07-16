use std::collections::BTreeSet;

use thiserror::Error;

use super::ScenarioDraft;

const MAX_PLANNING_HORIZON: u64 = 10_000;

pub(super) fn validate(value: &ScenarioDraft) -> Result<(), ScenarioError> {
    if value.name.trim().is_empty() {
        return Err(ScenarioError::EmptyName);
    }
    if value.title.trim().is_empty() {
        return Err(ScenarioError::EmptyTitle);
    }
    if value.planning_horizon == 0 {
        return Err(ScenarioError::ZeroPlanningHorizon);
    }
    if value.planning_horizon > MAX_PLANNING_HORIZON {
        return Err(ScenarioError::PlanningHorizonTooLarge);
    }
    unique_positive(
        value.objectives.iter().map(|item| item.outcome_id),
        value.objectives.iter().map(|item| item.importance),
        ScenarioError::DuplicateObjective,
        ScenarioError::InvalidImportance,
    )?;
    unique_ids(
        value.candidate_interventions.iter().copied(),
        ScenarioError::DuplicateCandidate,
    )?;
    unique_positive(
        value.budgets.iter().map(|item| item.unit.clone()),
        value.budgets.iter().map(|item| item.amount),
        ScenarioError::DuplicateBudget,
        ScenarioError::InvalidBudget,
    )?;
    if let Some(preferences) = &value.scalar_preferences {
        unique_positive(
            preferences.iter().map(|item| item.unit.clone()),
            preferences.iter().map(|item| item.utility_per_unit),
            ScenarioError::DuplicateScalarPreference,
            ScenarioError::InvalidScalarPreference,
        )?;
    }
    Ok(())
}

fn unique_ids<T: Ord>(
    values: impl Iterator<Item = T>,
    duplicate: ScenarioError,
) -> Result<(), ScenarioError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(duplicate);
        }
    }
    Ok(())
}

fn unique_positive<T: Ord>(
    keys: impl Iterator<Item = T>,
    values: impl Iterator<Item = f64>,
    duplicate: ScenarioError,
    invalid: ScenarioError,
) -> Result<(), ScenarioError> {
    let mut seen = BTreeSet::new();
    for (key, value) in keys.zip(values) {
        if !value.is_finite() || value <= 0.0 {
            return Err(invalid.clone());
        }
        if !seen.insert(key) {
            return Err(duplicate.clone());
        }
    }
    Ok(())
}

/// Aggregate-local scenario validation failures.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ScenarioError {
    /// The scenario name contains no visible text.
    #[error("a scenario name cannot be empty")]
    EmptyName,
    /// The display title contains no visible text.
    #[error("a scenario title cannot be empty")]
    EmptyTitle,
    /// Analysis requires at least one planning period.
    #[error("a scenario planning horizon must be nonzero")]
    ZeroPlanningHorizon,
    /// Dynamic analysis bounds the number of retained synchronous states.
    #[error("a scenario planning horizon cannot exceed 10,000 periods")]
    PlanningHorizonTooLarge,
    /// The same outcome appears more than once.
    #[error("scenario objectives must reference unique outcomes")]
    DuplicateObjective,
    /// Objective importance must be finite and positive.
    #[error("scenario objective importance must be finite and positive")]
    InvalidImportance,
    /// The same intervention appears more than once.
    #[error("scenario candidates must reference unique interventions")]
    DuplicateCandidate,
    /// The same budget unit appears more than once.
    #[error("scenario budgets must use unique units")]
    DuplicateBudget,
    /// Budget amounts must be finite and positive.
    #[error("scenario budget amounts must be finite and positive")]
    InvalidBudget,
    /// The same scalar conversion unit appears more than once.
    #[error("scenario scalar preferences must use unique units")]
    DuplicateScalarPreference,
    /// Scalar conversion values must be finite and positive.
    #[error("scenario scalar preferences must be finite and positive")]
    InvalidScalarPreference,
}
