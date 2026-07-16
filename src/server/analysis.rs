use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};

use crate::domain::{
    AnalysisLimits, ImpedimentAnalysis, ProjectId, ScenarioAnalysis, ScenarioId, StructuralAnalysis,
};
use crate::project::ProjectError;

use super::{AppState, api_error::ApiError};

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project}/analysis/structure",
            get(structure),
        )
        .route(
            "/api/v1/projects/{project}/analysis/impediments",
            get(impediments),
        )
        .route(
            "/api/v1/projects/{project}/scenarios/{scenario}/analysis",
            get(scenario),
        )
}

async fn impediments(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
) -> Result<Json<ImpedimentAnalysis>, ApiError> {
    Ok(Json(
        state.catalog.write().await.analyze_impediments(&project)?,
    ))
}

#[derive(serde::Deserialize)]
struct AnalysisQuery {
    scenario: Option<ScenarioId>,
    maximum_cycle_length: Option<usize>,
    maximum_cycles: Option<usize>,
}

async fn structure(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
    Query(query): Query<AnalysisQuery>,
) -> Result<Json<StructuralAnalysis>, ApiError> {
    let defaults = AnalysisLimits::default();
    let limits = AnalysisLimits::new(
        query
            .maximum_cycle_length
            .unwrap_or(defaults.maximum_cycle_length),
        query.maximum_cycles.unwrap_or(defaults.maximum_cycles),
    )
    .map_err(ProjectError::from)?;
    Ok(Json(state.catalog.write().await.analyze_structure(
        &project,
        query.scenario,
        limits,
    )?))
}

async fn scenario(
    State(state): State<AppState>,
    Path((project, scenario)): Path<(ProjectId, ScenarioId)>,
) -> Result<Json<ScenarioAnalysis>, ApiError> {
    Ok(Json(
        state
            .catalog
            .write()
            .await
            .analyze_scenario(&project, scenario)?,
    ))
}
