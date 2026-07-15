use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};

use crate::{
    domain::ProjectId,
    project::{CreateProject, Project},
};

use super::{AppState, api_error::ApiError};

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/projects", get(list).post(create))
        .route("/api/v1/projects/{project}", get(show).delete(delete))
}

async fn create(
    State(state): State<AppState>,
    Json(request): Json<CreateProject>,
) -> Result<(StatusCode, Json<Project>), ApiError> {
    let project = state.catalog.write().await.create(request.name)?;
    Ok((StatusCode::CREATED, Json(project)))
}

async fn list(State(state): State<AppState>) -> Json<Vec<Project>> {
    Json(state.catalog.read().await.list())
}

async fn show(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
) -> Result<Json<Project>, ApiError> {
    Ok(Json(state.catalog.read().await.get(&project)?))
}

async fn delete(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
) -> Result<Json<Project>, ApiError> {
    Ok(Json(state.catalog.write().await.delete(&project)?))
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

    async fn body(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn creates_lists_and_shows_projects() {
        let app = router();
        let create = app
            .clone()
            .oneshot(
                Request::post("/api/v1/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"name":"Delivery"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create.status(), StatusCode::CREATED);
        assert_eq!(body(create).await["id"], "A");

        let list = app
            .clone()
            .oneshot(
                Request::get("/api/v1/projects")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body(list).await.as_array().unwrap().len(), 1);

        let show = app
            .oneshot(
                Request::get("/api/v1/projects/A")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body(show).await["name"], "Delivery");
    }

    #[tokio::test]
    async fn returns_actionable_project_errors() {
        let response = router()
            .oneshot(
                Request::get("/api/v1/projects/A")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let error = body(response).await;
        assert_eq!(error["error"]["code"], "project_not_found");
        assert!(!error["error"]["advice"].as_array().unwrap().is_empty());
    }
}
