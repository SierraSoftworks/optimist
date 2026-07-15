use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;

use crate::project::ProjectError;
use crate::store::RepositoryError;

pub(super) struct ApiError {
    status: StatusCode,
    body: ErrorEnvelope,
}

#[derive(Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
    advice: Vec<&'static str>,
}

impl From<ProjectError> for ApiError {
    fn from(error: ProjectError) -> Self {
        let (status, code, advice): (_, _, &'static [&'static str]) = match &error {
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
            ProjectError::Repository(RepositoryError::DuplicateName(_)) => (
                StatusCode::CONFLICT,
                "node_name_conflict",
                &["Choose a node name or alias which is not already used in this project."],
            ),
            ProjectError::Repository(RepositoryError::MissingEntity(_)) => (
                StatusCode::NOT_FOUND,
                "node_not_found",
                &["List project nodes and retry with a returned entity ID."],
            ),
            ProjectError::Repository(RepositoryError::DuplicateEdge(_)) => (
                StatusCode::CONFLICT,
                "edge_conflict",
                &[
                    "Use `optimist edge get` to inspect the existing relationship before changing it.",
                ],
            ),
            ProjectError::Repository(RepositoryError::MissingEdge(_)) => (
                StatusCode::NOT_FOUND,
                "edge_not_found",
                &["List project edges and retry with a returned edge ID."],
            ),
            ProjectError::Repository(RepositoryError::InvalidEdge(_))
            | ProjectError::Repository(RepositoryError::EndpointKindMismatch { .. }) => (
                StatusCode::BAD_REQUEST,
                "invalid_edge",
                &["Check that the relationship kind is valid for both endpoint node kinds."],
            ),
            ProjectError::IdentifierSpaceExhausted
            | ProjectError::RevisionSpaceExhausted(_)
            | ProjectError::EdgeRevisionSpaceExhausted(_)
            | ProjectError::Repository(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "project_store_failure",
                &["Retry the request and inspect the server logs if the problem persists."],
            ),
        };
        Self {
            status,
            body: ErrorEnvelope {
                error: ErrorBody {
                    code,
                    message: error.to_string(),
                    advice: advice.to_vec(),
                },
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(self.body)).into_response()
    }
}
