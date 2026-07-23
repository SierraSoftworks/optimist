use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};

use crate::domain::{
    AnalysisLimits, ImpedimentAnalysis, ProjectId, ScenarioAnalysis, ScenarioId,
    SquiggleEstimateAssessment, SquiggleEstimateDefinition, SquiggleEstimateSupport,
    StructuralAnalysis, assess_squiggle_estimate,
};
use crate::project::{EstimateCommandError, ProjectError};

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
            "/api/v1/projects/{project}/analysis/squiggle-assessment",
            post(squiggle_assessment),
        )
}

#[derive(serde::Deserialize)]
struct SquiggleAssessmentRequest {
    definition: SquiggleEstimateDefinition,
    support: SquiggleEstimateSupport,
}

#[derive(serde::Serialize)]
struct SquiggleAssessmentResponse {
    assessment: SquiggleEstimateAssessment,
    effective_distribution: crate::domain::Distribution,
    predictive_checks: SquigglePredictiveChecks,
}

#[derive(serde::Serialize)]
struct SquigglePredictiveChecks {
    attempted_draws: usize,
    valid_draws: usize,
    invalid_draws: usize,
    support_violation_draws: usize,
    support_violation_probability: f64,
    representative_outcomes: Vec<RepresentativeOutcome>,
}

#[derive(serde::Serialize)]
struct RepresentativeOutcome {
    percentile: f64,
    value: f64,
}

async fn squiggle_assessment(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
    Json(request): Json<SquiggleAssessmentRequest>,
) -> Result<Json<SquiggleAssessmentResponse>, ApiError> {
    state.catalog.read().await.get(&project)?;
    let target_unit = request.definition.target_unit.clone();
    let support = request.support;
    let (_, assessment, effective_distribution) = state
        .analysis_worker
        .run(move || {
            assess_squiggle_estimate(request.definition, &target_unit)
                .map_err(EstimateCommandError::from)
                .map_err(ProjectError::from)
        })
        .await?;
    let predictive_checks = predictive_checks(&effective_distribution, &assessment, support);
    Ok(Json(SquiggleAssessmentResponse {
        assessment,
        effective_distribution,
        predictive_checks,
    }))
}

fn predictive_checks(
    distribution: &crate::domain::Distribution,
    assessment: &SquiggleEstimateAssessment,
    support: SquiggleEstimateSupport,
) -> SquigglePredictiveChecks {
    let representative_outcomes = [0.1, 0.5, 0.9]
        .into_iter()
        .map(|percentile| RepresentativeOutcome {
            percentile,
            value: distribution.quantile(percentile),
        })
        .collect();
    let retained = distribution.retained_draws();
    let valid_draws = retained.map_or(assessment.sample_count, <[f64]>::len);
    let support_violation_draws = retained.map_or_else(
        || {
            usize::from(
                assessment
                    .mean
                    .is_some_and(|value| !support_contains(support, value)),
            )
        },
        |draws| {
            draws
                .iter()
                .filter(|draw| !support_contains(support, **draw))
                .count()
        },
    );
    SquigglePredictiveChecks {
        attempted_draws: valid_draws,
        valid_draws,
        invalid_draws: 0,
        support_violation_draws,
        support_violation_probability: support_violation_draws as f64 / valid_draws as f64,
        representative_outcomes,
    }
}

fn support_contains(support: SquiggleEstimateSupport, value: f64) -> bool {
    match support {
        SquiggleEstimateSupport::Real => value.is_finite(),
        SquiggleEstimateSupport::NonNegative => value >= 0.0,
        SquiggleEstimateSupport::Probability => (0.0..=1.0).contains(&value),
        SquiggleEstimateSupport::Signed => (-1.0..=1.0).contains(&value),
        SquiggleEstimateSupport::Bounded { lower, upper } => (lower..=upper).contains(&value),
    }
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
    async fn evaluates_rich_squiggle_distributions_and_reports_diagnostics() {
        let response = app()
            .oneshot(
                Request::post("/api/v1/projects/A/analysis/squiggle-assessment")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "definition": {
                                "source": "gamma(4, 3) + triangular(0, 2, 5)",
                                "seed": 42,
                                "sample_count": 512,
                                "target_unit": {"day": 1}
                            },
                            "support": { "bounded": { "lower": 0.0, "upper": 30.0 } }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["effective_distribution"]["type"], "empirical");
        assert_eq!(
            value["effective_distribution"]["samples"]
                .as_array()
                .unwrap()
                .len(),
            512
        );
        assert!(
            value["assessment"]["p05"].as_f64().unwrap()
                < value["assessment"]["p95"].as_f64().unwrap()
        );
        assert_eq!(value["predictive_checks"]["attempted_draws"], 512);
        assert_eq!(value["predictive_checks"]["invalid_draws"], 0);
        assert!(
            value["predictive_checks"]["support_violation_draws"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert_eq!(
            value["predictive_checks"]["representative_outcomes"]
                .as_array()
                .unwrap()
                .len(),
            3
        );

        let invalid = app()
            .oneshot(
                Request::post("/api/v1/projects/A/analysis/squiggle-assessment")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "definition": {
                                "source": "missing + 1",
                                "seed": 42,
                                "sample_count": 512,
                                "target_unit": {}
                            },
                            "support": "real"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(invalid.into_body(), 16 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["error"]["code"], "invalid_squiggle_estimate");
    }
}
