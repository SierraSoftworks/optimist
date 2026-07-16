use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};

use crate::domain::{
    AnalysisLimits, FermiAssessment, FermiEstimateSupport, Formula, ImpedimentAnalysis,
    MonteCarloConfig, ProjectId, ScenarioAnalysis, ScenarioId, StructuralAnalysis, Unit,
    assess_fermi,
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
        .route(
            "/api/v1/projects/{project}/analysis/fermi-assessment",
            post(fermi_assessment),
        )
}

#[derive(serde::Deserialize)]
struct FermiAssessmentRequest {
    formula: Formula,
    support: FermiEstimateSupport,
    expected_unit: Unit,
    monte_carlo: MonteCarloConfig,
}

async fn fermi_assessment(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
    Json(request): Json<FermiAssessmentRequest>,
) -> Result<Json<FermiAssessment>, ApiError> {
    state.catalog.read().await.get(&project)?;
    Ok(Json(
        assess_fermi(
            &project,
            request.formula,
            request.support,
            request.expected_unit,
            request.monte_carlo,
        )
        .map_err(ProjectError::from)?,
    ))
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

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use serde_json::json;
    use tower::ServiceExt;

    use crate::{project::ProjectCatalog, server::router_with_catalog};

    fn app() -> axum::Router {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        assert_eq!(project.id.to_string(), "A");
        router_with_catalog(catalog)
    }

    #[tokio::test]
    async fn assesses_literal_fermi_decompositions() {
        let response = app()
            .oneshot(
                Request::post("/api/v1/projects/A/analysis/fermi-assessment")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "formula": {
                                "type": "product",
                                "factors": [
                                    { "type": "literal", "distribution": { "type": "scaled_beta", "alpha": 3.0, "beta": 3.0, "lower": 0.5, "upper": 0.9 }, "unit": {} },
                                    { "type": "literal", "distribution": { "type": "scaled_beta", "alpha": 4.0, "beta": 2.0, "lower": 0.6, "upper": 1.0 }, "unit": {} }
                                ]
                            },
                            "support": "probability",
                            "expected_unit": {},
                            "monte_carlo": { "seed": 42, "minimum_samples": 1000, "maximum_samples": 10000, "absolute_tolerance": 0.001, "relative_tolerance": 0.01 }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["recommendation"]["status"], "moment_matched");
        assert_eq!(value["recommendation"]["distribution"]["type"], "beta");
        assert!(
            value["report"]["diagnostics"]["valid_samples"]
                .as_u64()
                .unwrap()
                >= 1000
        );
    }

    #[tokio::test]
    async fn rejects_invalid_fermi_formulas_with_advice() {
        let response = app()
            .oneshot(
                Request::post("/api/v1/projects/A/analysis/fermi-assessment")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "formula": { "type": "sum", "terms": [{ "type": "literal", "distribution": { "type": "point", "value": 1.0 }, "unit": {} }] },
                            "support": "real",
                            "expected_unit": {},
                            "monte_carlo": { "seed": 42, "minimum_samples": 100, "maximum_samples": 1000, "absolute_tolerance": 0.001, "relative_tolerance": 0.01 }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["error"]["code"], "invalid_fermi_assessment");
        assert!(!value["error"]["advice"].as_array().unwrap().is_empty());
    }
}
