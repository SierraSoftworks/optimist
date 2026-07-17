use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::StatusCode,
    routing::get,
};

use crate::{
    command::ChangeSetReplay,
    domain::ProjectId,
    project::{CreateProject, Project, ProjectArchive},
};

use super::{AppState, api_error::ApiError};

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/projects", get(list).post(create))
        .route(
            "/api/v1/project-archives",
            axum::routing::post(import_archive)
                .layer(DefaultBodyLimit::max(crate::project::MAX_ARCHIVE_BYTES * 2)),
        )
        .route("/api/v1/projects/{project}", get(show).delete(delete))
        .route("/api/v1/projects/{project}/archive", get(export_archive))
        .route("/api/v1/projects/{project}/changes", get(changes))
}

async fn create(
    State(state): State<AppState>,
    Json(request): Json<CreateProject>,
) -> Result<(StatusCode, Json<Project>), ApiError> {
    let project = state.mutate(|catalog| catalog.create(request.name)).await?;
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
    Ok(Json(
        state.mutate(|catalog| catalog.delete(&project)).await?,
    ))
}

async fn export_archive(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
) -> Result<Json<ProjectArchive>, ApiError> {
    Ok(Json(state.catalog.write().await.export_archive(&project)?))
}

#[derive(serde::Deserialize)]
struct ImportQuery {
    #[serde(default)]
    replace: bool,
    #[serde(default)]
    yes: bool,
}

async fn import_archive(
    State(state): State<AppState>,
    Query(query): Query<ImportQuery>,
    Json(archive): Json<ProjectArchive>,
) -> Result<(StatusCode, Json<Project>), ApiError> {
    let project = state
        .mutate(|catalog| catalog.import_archive(&archive, query.replace, query.yes))
        .await?;
    Ok((StatusCode::CREATED, Json(project)))
}

#[derive(serde::Deserialize)]
struct ReplayQuery {
    after: u64,
}

async fn changes(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
    Query(query): Query<ReplayQuery>,
) -> Result<Json<ChangeSetReplay>, ApiError> {
    Ok(Json(
        state
            .catalog
            .write()
            .await
            .replay_changes_with_snapshot(&project, query.after)?,
    ))
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

    #[tokio::test]
    async fn exports_imports_and_confirms_project_replacement() {
        let app = router();
        let created = app
            .clone()
            .oneshot(
                Request::post("/api/v1/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"name":"Delivery"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);
        let command = app
            .clone()
            .oneshot(
                Request::post("/api/v1/projects/A/commands")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "request_id": "00000000-0000-4000-8000-000000000001",
                            "expected_revision": 0,
                            "command": {
                                "type": "create_node",
                                "payload": {
                                    "name": "flow",
                                    "title": "Flow",
                                    "payload": {
                                        "kind": "factor",
                                        "properties": {
                                            "current": null,
                                            "desired": null,
                                            "controllable": false,
                                            "evidence": []
                                        }
                                    }
                                }
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(command.status(), StatusCode::CREATED);
        let archive = app
            .clone()
            .oneshot(
                Request::get("/api/v1/projects/A/archive")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let archive = body(archive).await;
        assert!(archive["files"]["_project.md"].is_string());

        let conflict = app
            .clone()
            .oneshot(
                Request::post("/api/v1/project-archives")
                    .header("content-type", "application/json")
                    .body(Body::from(archive.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        assert_eq!(
            body(conflict).await["error"]["code"],
            "project_import_requires_replace"
        );

        let replaced = app
            .clone()
            .oneshot(
                Request::post("/api/v1/project-archives?replace=true&yes=true")
                    .header("content-type", "application/json")
                    .body(Body::from(archive.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(replaced.status(), StatusCode::CREATED);
        assert_eq!(body(replaced).await["id"], "A");

        let fallback = app
            .oneshot(
                Request::get("/api/v1/projects/A/changes?after=0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(fallback.status(), StatusCode::OK);
        let fallback = body(fallback).await;
        assert_eq!(fallback["current_revision"], archive["project"]["revision"]);
        assert_eq!(
            fallback["snapshot"]["revision"],
            archive["project"]["revision"]
        );
        assert_eq!(fallback["snapshot"]["archive"], archive);
        assert!(fallback["changes"].as_array().unwrap().is_empty());
    }
}
