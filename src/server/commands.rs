use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::post,
};

use crate::{
    command::{
        CommandBatchRequest, CommandBatchResult, CommandRequest, CommandResult,
        CompensatingUndoRequest,
    },
    domain::ProjectId,
};

use super::{AppState, api_error::ApiError};

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/projects/{project}/commands", post(execute))
        .route(
            "/api/v1/projects/{project}/command-batches",
            post(execute_batch),
        )
        .route(
            "/api/v1/projects/{project}/command-batches/{batch}/undo",
            post(undo_batch),
        )
}

async fn execute_batch(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
    Json(request): Json<CommandBatchRequest>,
) -> Result<(StatusCode, Json<CommandBatchResult>), ApiError> {
    commit_batch(state, project, request, None).await
}

async fn undo_batch(
    State(state): State<AppState>,
    Path((project, batch)): Path<(ProjectId, uuid::Uuid)>,
    Json(request): Json<CompensatingUndoRequest>,
) -> Result<(StatusCode, Json<CommandBatchResult>), ApiError> {
    commit_batch(
        state,
        project,
        CommandBatchRequest {
            request_id: request.request_id,
            expected_revision: request.expected_revision,
            commands: request.commands,
        },
        Some(batch),
    )
    .await
}

async fn commit_batch(
    state: AppState,
    project: ProjectId,
    request: CommandBatchRequest,
    compensates: Option<uuid::Uuid>,
) -> Result<(StatusCode, Json<CommandBatchResult>), ApiError> {
    let (result, changes) = state.execute_batch(&project, request, compensates).await?;
    for change in changes {
        state.publish(&project, change).await;
    }
    Ok((StatusCode::CREATED, Json(result)))
}

async fn execute(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
    Json(request): Json<CommandRequest>,
) -> Result<(StatusCode, Json<CommandResult>), ApiError> {
    let (result, changes) = state.execute_command(&project, request).await?;
    for (changed_project, change) in changes {
        state.publish(&changed_project, change).await;
    }
    Ok((StatusCode::CREATED, Json(result)))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use crate::server::router;

    const BATCH: &str = "00000000-0000-4000-8000-000000000010";
    const UNDO: &str = "00000000-0000-4000-8000-000000000020";

    async fn body(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn post(app: &axum::Router, uri: &str, value: &Value) -> axum::response::Response {
        app.clone()
            .oneshot(
                Request::post(uri)
                    .header("content-type", "application/json")
                    .body(Body::from(value.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    fn create_factor(name: &str) -> Value {
        json!({
            "type": "create_node",
            "payload": {
                "name": name,
                "title": name,
                "payload": {
                    "kind": "factor",
                    "properties": {
                        "controllable": false,
                        "evidence": []
                    }
                }
            }
        })
    }

    #[tokio::test]
    async fn commits_retries_and_compensates_batches_atomically() {
        let app = router();
        assert_eq!(
            post(&app, "/api/v1/projects", &json!({"name":"Delivery"}))
                .await
                .status(),
            StatusCode::CREATED
        );
        let batch = json!({
            "request_id": BATCH,
            "expected_revision": 0,
            "commands": [create_factor("flow"), create_factor("quality")]
        });
        let first = post(&app, "/api/v1/projects/A/command-batches", &batch).await;
        assert_eq!(first.status(), StatusCode::CREATED);
        let first = body(first).await;
        assert_eq!(first["base_revision"], 0);
        assert_eq!(first["project_revision"], 2);
        assert_eq!(first["results"].as_array().unwrap().len(), 2);
        assert_eq!(
            body(post(&app, "/api/v1/projects/A/command-batches", &batch).await).await,
            first
        );

        let invalid = json!({
            "request_id": "00000000-0000-4000-8000-000000000011",
            "expected_revision": 2,
            "commands": [create_factor("temporary"), create_factor("flow")]
        });
        assert_eq!(
            post(&app, "/api/v1/projects/A/command-batches", &invalid)
                .await
                .status(),
            StatusCode::CONFLICT
        );
        let nodes = app
            .clone()
            .oneshot(
                Request::get("/api/v1/projects/A/nodes")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body(nodes).await.as_array().unwrap().len(), 2);

        let undo = json!({
            "request_id": UNDO,
            "expected_revision": 2,
            "commands": [
                {"type":"delete_node","payload":{"id":"B"}},
                {"type":"delete_node","payload":{"id":"A"}}
            ]
        });
        let undone = post(
            &app,
            &format!("/api/v1/projects/A/command-batches/{BATCH}/undo"),
            &undo,
        )
        .await;
        assert_eq!(undone.status(), StatusCode::CREATED);
        let undone = body(undone).await;
        assert_eq!(undone["compensates"], BATCH);
        assert_eq!(undone["project_revision"], 4);

        let second_undo = json!({
            "request_id": "00000000-0000-4000-8000-000000000021",
            "expected_revision": 4,
            "commands": [create_factor("replacement")]
        });
        let conflict = post(
            &app,
            &format!("/api/v1/projects/A/command-batches/{BATCH}/undo"),
            &second_undo,
        )
        .await;
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        assert_eq!(
            body(conflict).await["error"]["code"],
            "command_batch_conflict"
        );

        let replay = app
            .oneshot(
                Request::get("/api/v1/projects/A/changes?after=0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let replay = body(replay).await;
        assert_eq!(replay["changes"].as_array().unwrap().len(), 4);
        assert_eq!(replay["changes"][0]["batch_id"], BATCH);
        assert_eq!(replay["changes"][2]["compensates"], BATCH);
        assert_eq!(replay["changes"][2]["batch_id"], UNDO);
    }
}
