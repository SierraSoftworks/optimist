use axum::http::StatusCode;

use crate::project::ProjectError;

use super::{estimate_error_response, formula_error_response, repository_error_response};

pub(super) fn classify(
    error: &ProjectError,
) -> (StatusCode, &'static str, &'static [&'static str]) {
    match error {
        ProjectError::EmptyName => (
            StatusCode::BAD_REQUEST,
            "invalid_project_name",
            &["Provide a non-empty project name."],
        ),
        ProjectError::DuplicateName(_) => (
            StatusCode::CONFLICT,
            "project_name_conflict",
            &["Choose a project name which is not already in use."],
        ),
        ProjectError::NotFound(_) => (
            StatusCode::NOT_FOUND,
            "project_not_found",
            &["List projects and retry with one of the returned project IDs."],
        ),
        ProjectError::RevisionConflict { .. } => (
            StatusCode::CONFLICT,
            "project_revision_conflict",
            &["Refresh the project and rebuild the command against its current revision."],
        ),
        ProjectError::Node(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_node",
            &["Provide a non-empty node name and title with fields valid for its node kind."],
        ),
        ProjectError::EdgeId(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_edge_id",
            &["Use an edge ID returned by `optimist edge list`, such as `A-requires-B`."],
        ),
        ProjectError::EstimateAddress(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_estimate_address",
            &[
                "Use `<project>/<node|edge>/<owner>/estimate/<id>` with canonical project, owner, and estimate IDs.",
            ],
        ),
        ProjectError::NotMeasurementEdge(_) => (
            StatusCode::BAD_REQUEST,
            "not_measurement_edge",
            &["Choose a `measures` edge returned by `optimist edge list`."],
        ),
        ProjectError::ObservationUnitMismatch { .. } => (
            StatusCode::BAD_REQUEST,
            "observation_unit_mismatch",
            &["Use the unit defined by the measurement edge's source metric."],
        ),
        ProjectError::Observation(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_observation",
            &[
                "Check the value, RFC 3339 timestamp, source, unit, and measurement standard deviation.",
            ],
        ),
        ProjectError::Scenario(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_scenario",
            &[
                "Check the name, title, horizon, unique references, positive weights and budgets, and Monte Carlo configuration.",
            ],
        ),
        ProjectError::ScenarioNotFound(_) => (
            StatusCode::NOT_FOUND,
            "scenario_not_found",
            &["List scenarios and retry with one of the returned scenario IDs."],
        ),
        ProjectError::ScenarioRevisionConflict { .. } => (
            StatusCode::CONFLICT,
            "scenario_revision_conflict",
            &["Show the scenario and retry with its current scenario and project revisions."],
        ),
        ProjectError::InvalidScenarioReference { .. } => (
            StatusCode::BAD_REQUEST,
            "invalid_scenario_reference",
            &["Use outcome IDs for objectives and intervention IDs for candidate interventions."],
        ),
        ProjectError::Dependence(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_dependence",
            &[
                "Check unique same-project members and provide a finite symmetric positive-semidefinite correlation matrix.",
            ],
        ),
        ProjectError::DependenceNotFound(_) => (
            StatusCode::NOT_FOUND,
            "dependence_not_found",
            &["Set a project dependence document before trying to show or remove it."],
        ),
        ProjectError::DependenceRevisionConflict { .. } => (
            StatusCode::CONFLICT,
            "dependence_revision_conflict",
            &[
                "Show project dependence and retry with its current dependence and project revisions.",
            ],
        ),
        ProjectError::MissingEstimateAddress(_) => (
            StatusCode::BAD_REQUEST,
            "missing_estimate_address",
            &["Use estimate addresses embedded in existing project nodes or edges."],
        ),
        ProjectError::EstimateCommand(error) => estimate_error_response::classify(error),
        ProjectError::FormulaCommand(error) => formula_error_response::classify(error),
        ProjectError::IdentifierSpaceExhausted
        | ProjectError::RevisionSpaceExhausted(_)
        | ProjectError::EdgeRevisionSpaceExhausted(_)
        | ProjectError::ScenarioRevisionSpaceExhausted(_)
        | ProjectError::ScenarioIdentifierSpaceExhausted(_)
        | ProjectError::DependenceRevisionSpaceExhausted(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "project_store_failure",
            &["Retry the request and inspect the server logs if the problem persists."],
        ),
        ProjectError::Repository(error) => repository_error_response::classify(error),
    }
}
