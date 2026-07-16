use axum::http::StatusCode;

use crate::store::RepositoryError;

pub(super) fn classify(
    error: &RepositoryError,
) -> (StatusCode, &'static str, &'static [&'static str]) {
    match error {
        RepositoryError::DuplicateName(_) => (
            StatusCode::CONFLICT,
            "node_name_conflict",
            &["Choose a node name or alias which is not already used in this project."],
        ),
        RepositoryError::MissingEntity(_) => (
            StatusCode::NOT_FOUND,
            "node_not_found",
            &["List project nodes and retry with a returned entity ID."],
        ),
        RepositoryError::EntityHasEdges(_) => (
            StatusCode::CONFLICT,
            "node_has_edges",
            &[
                "List the project's edges, delete every relationship connected to this node, then retry the node deletion.",
            ],
        ),
        RepositoryError::DuplicateEdge(_) => (
            StatusCode::CONFLICT,
            "edge_conflict",
            &["Use `optimist edge get` to inspect the existing relationship before changing it."],
        ),
        RepositoryError::MissingEdge(_) => (
            StatusCode::NOT_FOUND,
            "edge_not_found",
            &["List project edges and retry with a returned edge ID."],
        ),
        RepositoryError::InvalidEdge(_) | RepositoryError::EndpointKindMismatch { .. } => (
            StatusCode::BAD_REQUEST,
            "invalid_edge",
            &["Check that the relationship kind is valid for both endpoint node kinds."],
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "project_store_failure",
            &["Retry the request and inspect the server logs if the problem persists."],
        ),
    }
}
