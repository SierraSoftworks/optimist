use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};

use crate::{
    domain::{EstimateAddress, FormulaCatalog, FormulaDefinition, ProjectId},
    project::ProjectError,
};

use super::{AppState, api_error::ApiError};

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/projects/{project}/formulas", get(list))
        .route("/api/v1/projects/{project}/formula", get(show))
}

async fn list(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
) -> Result<Json<FormulaCatalog>, ApiError> {
    Ok(Json(state.catalog.write().await.list_formulas(&project)?))
}

#[derive(serde::Deserialize)]
struct FormulaQuery {
    address: String,
}

async fn show(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
    Query(query): Query<FormulaQuery>,
) -> Result<Json<FormulaDefinition>, ApiError> {
    let address = query
        .address
        .parse::<EstimateAddress>()
        .map_err(ProjectError::from)?;
    Ok(Json(
        state
            .catalog
            .write()
            .await
            .get_formula(&project, &address)?,
    ))
}
