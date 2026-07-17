use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    domain::ProjectId,
    project::{CatalogBackup, CatalogRestore, ProjectArchive, ProjectSnapshot},
};

use super::{AppState, api_error::ApiError};

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/backups", get(list_backups).post(create_backup))
        .route("/api/v1/backups/{backup}/restore", post(restore_backup))
        .route(
            "/api/v1/projects/{project}/snapshots",
            get(list_project_snapshots).post(create_project_snapshot),
        )
        .route(
            "/api/v1/projects/{project}/snapshots/{revision}",
            get(get_project_snapshot),
        )
}

async fn create_backup(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<CatalogBackup>), ApiError> {
    Ok((StatusCode::CREATED, Json(state.create_backup().await?)))
}

async fn list_backups(State(state): State<AppState>) -> Result<Json<Vec<CatalogBackup>>, ApiError> {
    Ok(Json(state.list_backups()?))
}

#[derive(serde::Deserialize)]
struct RestoreQuery {
    #[serde(default)]
    yes: bool,
}

async fn restore_backup(
    State(state): State<AppState>,
    Path(backup): Path<Uuid>,
    Query(query): Query<RestoreQuery>,
) -> Result<Json<CatalogRestore>, ApiError> {
    Ok(Json(state.restore_backup(backup, query.yes).await?))
}

async fn create_project_snapshot(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
) -> Result<(StatusCode, Json<ProjectSnapshot>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.create_project_snapshot(&project).await?),
    ))
}

async fn list_project_snapshots(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
) -> Result<Json<Vec<ProjectSnapshot>>, ApiError> {
    Ok(Json(state.list_project_snapshots(&project)?))
}

async fn get_project_snapshot(
    State(state): State<AppState>,
    Path((project, revision)): Path<(ProjectId, u64)>,
) -> Result<Json<ProjectArchive>, ApiError> {
    Ok(Json(state.get_project_snapshot(&project, revision)?))
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
        project::{CatalogStore, ProjectCatalog},
        server::router_with_persistent_catalog,
    };

    async fn body(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn post(app: &axum::Router, uri: &str, value: Value) -> axum::response::Response {
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

    #[tokio::test]
    async fn restores_a_backup_and_preserves_the_replaced_catalog() {
        let root =
            std::env::temp_dir().join(format!("optimist-backup-api-{}", uuid::Uuid::new_v4()));
        let app =
            router_with_persistent_catalog(ProjectCatalog::new(), CatalogStore::new(root.clone()));

        assert_eq!(
            post(&app, "/api/v1/projects", json!({"name":"Delivery"}))
                .await
                .status(),
            StatusCode::CREATED
        );

        let snapshot = post(&app, "/api/v1/projects/A/snapshots", json!({})).await;
        assert_eq!(snapshot.status(), StatusCode::CREATED);
        let snapshot = body(snapshot).await;
        assert_eq!(snapshot["project"], "A");
        assert_eq!(snapshot["revision"], 0);

        let repeated = body(post(&app, "/api/v1/projects/A/snapshots", json!({})).await).await;
        assert_eq!(repeated, snapshot);
        let archived = app
            .clone()
            .oneshot(
                Request::get("/api/v1/projects/A/snapshots/0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(archived.status(), StatusCode::OK);
        assert_eq!(body(archived).await["project"]["id"], "A");

        let backup = post(&app, "/api/v1/backups", json!({})).await;
        assert_eq!(backup.status(), StatusCode::CREATED);
        let backup = body(backup).await;
        let backup_id = backup["id"].as_str().unwrap();
        assert_eq!(backup["projects"].as_array().unwrap().len(), 1);

        assert_eq!(
            post(&app, "/api/v1/projects", json!({"name":"Capacity"}))
                .await
                .status(),
            StatusCode::CREATED
        );

        let unconfirmed = post(
            &app,
            &format!("/api/v1/backups/{backup_id}/restore"),
            json!({}),
        )
        .await;
        assert_eq!(unconfirmed.status(), StatusCode::CONFLICT);
        assert_eq!(
            body(unconfirmed).await["error"]["code"],
            "backup_restore_requires_confirmation"
        );

        let restored = post(
            &app,
            &format!("/api/v1/backups/{backup_id}/restore?yes=true"),
            json!({}),
        )
        .await;
        assert_eq!(restored.status(), StatusCode::OK);
        let restored = body(restored).await;
        assert_eq!(restored["projects"].as_array().unwrap().len(), 1);
        assert_eq!(
            restored["safety_backup"]["projects"]
                .as_array()
                .unwrap()
                .len(),
            2
        );

        let projects = app
            .clone()
            .oneshot(
                Request::get("/api/v1/projects")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let projects = body(projects).await;
        assert_eq!(projects.as_array().unwrap().len(), 1);
        assert_eq!(projects[0]["name"], "Delivery");

        let backups = app
            .oneshot(Request::get("/api/v1/backups").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(body(backups).await.as_array().unwrap().len(), 2);

        std::fs::remove_dir_all(root).unwrap();
    }
}
