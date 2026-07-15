use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};

use crate::{
    command::{CommandRequest, CommandResult},
    domain::{EntityId, Node, ProjectId},
    project::ProjectError,
    store::RepositoryError,
};

use super::{AppState, api_error::ApiError};

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/projects/{project}/commands", post(execute_command))
        .route("/api/v1/projects/{project}/nodes", get(list_nodes))
        .route("/api/v1/projects/{project}/nodes/{entity}", get(show_node))
}

async fn execute_command(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
    Json(request): Json<CommandRequest>,
) -> Result<(StatusCode, Json<CommandResult>), ApiError> {
    let result = state.catalog.write().await.execute(&project, request)?;
    Ok((StatusCode::CREATED, Json(result)))
}

async fn list_nodes(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
) -> Result<Json<Vec<Node>>, ApiError> {
    Ok(Json(state.catalog.write().await.list_nodes(&project)?))
}

async fn show_node(
    State(state): State<AppState>,
    Path((project, entity)): Path<(ProjectId, EntityId)>,
) -> Result<Json<Node>, ApiError> {
    state
        .catalog
        .write()
        .await
        .get_node(&project, entity)?
        .map(Json)
        .ok_or_else(|| ProjectError::Repository(RepositoryError::MissingEntity(entity)).into())
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use crate::{
        command::{CommandRequest, CreateNode, GraphCommand},
        domain::{Factor, NodePayload},
        server::router,
    };

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), 16_384).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn create_project(app: &axum::Router) {
        app.clone()
            .oneshot(
                Request::post("/api/v1/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"name":"Delivery"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    fn command(revision: u64) -> CommandRequest {
        CommandRequest::new(
            revision,
            GraphCommand::CreateNode(CreateNode {
                name: "github".to_owned(),
                title: "GitHub".to_owned(),
                payload: NodePayload::Factor(Factor {
                    current: None,
                    desired: None,
                    controllable: false,
                    evidence: vec![],
                }),
            }),
        )
    }

    #[tokio::test]
    async fn executes_idempotent_commands_and_reads_nodes() {
        let app = router();
        create_project(&app).await;
        let command = command(0);
        let request = || {
            Request::post("/api/v1/projects/A/commands")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&command).unwrap()))
                .unwrap()
        };
        let first = app.clone().oneshot(request()).await.unwrap();
        let retry = app.clone().oneshot(request()).await.unwrap();
        assert_eq!(first.status(), StatusCode::CREATED);
        assert_eq!(response_json(first).await, response_json(retry).await);

        let node = app
            .clone()
            .oneshot(
                Request::get("/api/v1/projects/A/nodes/A")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response_json(node).await["title"], "GitHub");
    }

    #[tokio::test]
    async fn rejects_stale_commands_with_actionable_conflict() {
        let app = router();
        create_project(&app).await;
        for expected_status in [StatusCode::CREATED, StatusCode::CONFLICT] {
            let response = app
                .clone()
                .oneshot(
                    Request::post("/api/v1/projects/A/commands")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&command(0)).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), expected_status);
        }
    }
}
