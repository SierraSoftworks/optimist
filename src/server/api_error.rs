use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;

use crate::project::ProjectError;

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
            ProjectError::IdentifierSpaceExhausted | ProjectError::Repository(_) => (
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
