use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::post,
};

use crate::{
    command::{CommandRequest, CommandResult},
    domain::ProjectId,
};

use super::{AppState, api_error::ApiError};

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/projects/{project}/commands", post(execute))
}

async fn execute(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
    Json(request): Json<CommandRequest>,
) -> Result<(StatusCode, Json<CommandResult>), ApiError> {
    let command_project = project.clone();
    let (result, change) = state
        .mutate(move |catalog| {
            let before = catalog.get(&command_project)?.revision;
            let result = catalog.execute(&command_project, request)?;
            let change = if result.project_revision > before {
                catalog.get_change(&command_project, result.project_revision)?
            } else {
                None
            };
            Ok((result, change))
        })
        .await?;
    if let Some(change) = change {
        state.publish(&project, change).await;
    }
    Ok((StatusCode::CREATED, Json(result)))
}
