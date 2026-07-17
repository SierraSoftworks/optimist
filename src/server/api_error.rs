use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;

use crate::project::{BackupError, ProjectError};

use super::project_error_response;
use super::state::CatalogMutationError;

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
        let (status, code, advice) = project_error_response::classify(&error);
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

impl From<CatalogMutationError> for ApiError {
    fn from(error: CatalogMutationError) -> Self {
        match error {
            CatalogMutationError::Project(error) => Self::from(error),
            CatalogMutationError::Persistence(error) => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                body: ErrorEnvelope {
                    error: ErrorBody {
                        code: "catalog_persistence_failure",
                        message: error.to_string(),
                        advice: vec![
                            "Check that the server data directory is writable and has free space, then retry.",
                        ],
                    },
                },
            },
        }
    }
}

impl From<BackupError> for ApiError {
    fn from(error: BackupError) -> Self {
        match error {
            BackupError::Project(error) => Self::from(error),
            BackupError::Unavailable => Self::backup(
                StatusCode::SERVICE_UNAVAILABLE,
                "backup_storage_unavailable",
                error.to_string(),
                vec!["Start the server with a persistent data directory before using backup APIs."],
            ),
            BackupError::ConfirmationRequired => Self::backup(
                StatusCode::CONFLICT,
                "backup_restore_requires_confirmation",
                error.to_string(),
                vec![
                    "Repeat the restore request with `yes=true` after confirming the selected backup.",
                ],
            ),
            BackupError::BackupNotFound(_) => Self::backup(
                StatusCode::NOT_FOUND,
                "backup_not_found",
                error.to_string(),
                vec!["List catalog backups and retry with an available backup ID."],
            ),
            BackupError::SnapshotNotFound { .. } => Self::backup(
                StatusCode::NOT_FOUND,
                "project_snapshot_not_found",
                error.to_string(),
                vec!["List snapshots for the project and retry with an available revision."],
            ),
            BackupError::Io { .. } | BackupError::Json { .. } | BackupError::Catalog(_) => {
                Self::backup(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "backup_storage_failure",
                    error.to_string(),
                    vec![
                        "Inspect the server data directory and logs before retrying the backup operation.",
                    ],
                )
            }
        }
    }
}

impl ApiError {
    fn backup(
        status: StatusCode,
        code: &'static str,
        message: String,
        advice: Vec<&'static str>,
    ) -> Self {
        Self {
            status,
            body: ErrorEnvelope {
                error: ErrorBody {
                    code,
                    message,
                    advice,
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

#[cfg(test)]
mod tests {
    use axum::{body::to_bytes, response::IntoResponse};

    use crate::{domain::EntityId, project::ProjectError, store::RepositoryError};

    use super::ApiError;

    #[tokio::test]
    async fn incident_edges_return_an_actionable_conflict() {
        let response = ApiError::from(ProjectError::Repository(RepositoryError::EntityHasEdges(
            EntityId::new(0),
        )))
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["error"]["code"], "node_has_edges");
        assert!(!value["error"]["advice"].as_array().unwrap().is_empty());
    }
}
