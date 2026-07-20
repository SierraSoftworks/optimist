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
    let (result, changes) = state.execute_command(&project, request).await?;
    for (changed_project, change) in changes {
        state.publish(&changed_project, change).await;
    }
    Ok((StatusCode::CREATED, Json(result)))
}
