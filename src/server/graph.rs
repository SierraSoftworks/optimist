use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};

use crate::{
    domain::{
        Edge, EdgeId, EntityId, EstimateAddress, Node, PrimitiveEstimate, ProjectDependenceModel,
        ProjectId, Scenario, ScenarioId,
    },
    project::ProjectError,
    store::RepositoryError,
};

use super::{AppState, api_error::ApiError, formulas};

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/projects/{project}/nodes", get(list_nodes))
        .route("/api/v1/projects/{project}/nodes/{entity}", get(show_node))
        .route("/api/v1/projects/{project}/edges", get(list_edges))
        .route("/api/v1/projects/{project}/edges/{edge}", get(show_edge))
        .route("/api/v1/projects/{project}/estimates", get(show_estimate))
        .merge(formulas::router())
        .route("/api/v1/projects/{project}/scenarios", get(list_scenarios))
        .route(
            "/api/v1/projects/{project}/scenarios/{scenario}",
            get(show_scenario),
        )
        .route(
            "/api/v1/projects/{project}/dependence",
            get(show_dependence),
        )
}

async fn list_nodes(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
) -> Result<Json<Vec<Node>>, ApiError> {
    Ok(Json(state.catalog.write().await.list_nodes(&project)?))
}

async fn show_node(
    State(state): State<AppState>,
    Path((project, entity)): Path<(ProjectId, EntityId)>,
) -> Result<Json<Node>, ApiError> {
    state
        .catalog
        .write()
        .await
        .get_node(&project, entity)?
        .map(Json)
        .ok_or_else(|| ProjectError::Repository(RepositoryError::MissingEntity(entity)).into())
}

async fn list_edges(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
) -> Result<Json<Vec<Edge>>, ApiError> {
    Ok(Json(state.catalog.write().await.list_edges(&project)?))
}

async fn show_edge(
    State(state): State<AppState>,
    Path((project, edge)): Path<(ProjectId, String)>,
) -> Result<Json<Edge>, ApiError> {
    let edge_id: EdgeId = edge.parse::<EdgeId>().map_err(ProjectError::from)?;
    state
        .catalog
        .write()
        .await
        .get_edge(&project, &edge_id)?
        .map(Json)
        .ok_or_else(|| ProjectError::Repository(RepositoryError::MissingEdge(edge)).into())
}

#[derive(serde::Deserialize)]
struct EstimateQuery {
    address: String,
}

async fn show_estimate(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
    Query(query): Query<EstimateQuery>,
) -> Result<Json<PrimitiveEstimate>, ApiError> {
    let address = query
        .address
        .parse::<EstimateAddress>()
        .map_err(ProjectError::from)?;
    Ok(Json(
        state
            .catalog
            .write()
            .await
            .get_estimate(&project, &address)?,
    ))
}

async fn list_scenarios(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
) -> Result<Json<Vec<Scenario>>, ApiError> {
    Ok(Json(state.catalog.read().await.list_scenarios(&project)?))
}

async fn show_scenario(
    State(state): State<AppState>,
    Path((project, scenario)): Path<(ProjectId, ScenarioId)>,
) -> Result<Json<Scenario>, ApiError> {
    state
        .catalog
        .read()
        .await
        .get_scenario(&project, scenario)?
        .map(Json)
        .ok_or_else(|| ProjectError::ScenarioNotFound(scenario).into())
}

async fn show_dependence(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
) -> Result<Json<ProjectDependenceModel>, ApiError> {
    state
        .catalog
        .read()
        .await
        .get_dependence(&project)?
        .map(Json)
        .ok_or_else(|| ProjectError::DependenceNotFound(project).into())
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
        command::{
            CommandRequest, CreateEdge, CreateNode, CreateScenario, GraphCommand,
            RemoveProjectDependence, SetNodeQuantityState, SetProjectDependence,
        },
        domain::{
            EdgePayload, EntityId, Factor, LinearResponse, MonteCarloConfig, NodePayload,
            ProjectDependenceModel, QuantityDefinition, QuantitySupport, QuantityValue,
            ScenarioDraft, Unit,
        },
        server::router,
    };

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), 16_384).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn create_project(app: &axum::Router) {
        app.clone()
            .oneshot(
                Request::post("/api/v1/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"name":"Delivery"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    fn command(revision: u64) -> CommandRequest {
        CommandRequest::new(
            revision,
            GraphCommand::CreateNode(CreateNode {
                name: "github".to_owned(),
                title: "GitHub".to_owned(),
                payload: NodePayload::Factor(Factor {
                    controllable: false,
                    evidence: vec![],
                }),
            }),
        )
    }

    #[tokio::test]
    async fn executes_idempotent_commands_and_reads_nodes() {
        let app = router();
        create_project(&app).await;
        let command = command(0);
        let request = || {
            Request::post("/api/v1/projects/A/commands")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&command).unwrap()))
                .unwrap()
        };
        let first = app.clone().oneshot(request()).await.unwrap();
        let retry = app.clone().oneshot(request()).await.unwrap();
        assert_eq!(first.status(), StatusCode::CREATED);
        assert_eq!(response_json(first).await, response_json(retry).await);

        let node = app
            .clone()
            .oneshot(
                Request::get("/api/v1/projects/A/nodes/A")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response_json(node).await["title"], "GitHub");
    }

    #[tokio::test]
    async fn rejects_stale_commands_with_actionable_conflict() {
        let app = router();
        create_project(&app).await;
        for expected_status in [StatusCode::CREATED, StatusCode::CONFLICT] {
            let response = app
                .clone()
                .oneshot(
                    Request::post("/api/v1/projects/A/commands")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&command(0)).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), expected_status);
        }
    }

    #[tokio::test]
    async fn creates_lists_and_shows_scenarios() {
        let app = router();
        create_project(&app).await;
        let request = CommandRequest::new(
            0,
            GraphCommand::CreateScenario(CreateScenario {
                scenario: ScenarioDraft {
                    name: "empty graph".to_owned(),
                    title: "Empty graph".to_owned(),
                    rationale: "Transport fixture without references.".to_owned(),
                    objectives: vec![],
                    planning_horizon: 1,
                    budgets: vec![],
                    candidate_interventions: vec![],
                    monte_carlo: MonteCarloConfig::new(1, 2, 10, 0.1, 0.1).unwrap(),
                    scalar_preferences: None,
                },
            }),
        );
        let created = app
            .clone()
            .oneshot(
                Request::post("/api/v1/projects/A/commands")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);
        assert_eq!(response_json(created).await["outcome"]["value"]["id"], "A");

        let listed = app
            .clone()
            .oneshot(
                Request::get("/api/v1/projects/A/scenarios")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response_json(listed).await[0]["name"], "empty graph");

        let missing = app
            .oneshot(
                Request::get("/api/v1/projects/A/scenarios/B")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response_json(missing).await["error"]["code"],
            "scenario_not_found"
        );
    }

    #[tokio::test]
    async fn analyzes_scenarios_over_http() {
        let app = router();
        create_project(&app).await;
        let request = CommandRequest::new(
            0,
            GraphCommand::CreateScenario(CreateScenario {
                scenario: ScenarioDraft {
                    name: "empty graph".to_owned(),
                    title: "Empty graph".to_owned(),
                    rationale: String::new(),
                    objectives: vec![],
                    planning_horizon: 3,
                    budgets: vec![],
                    candidate_interventions: vec![],
                    monte_carlo: MonteCarloConfig::new(1, 2, 10, 0.1, 0.1).unwrap(),
                    scalar_preferences: None,
                },
            }),
        );
        app.clone()
            .oneshot(
                Request::post("/api/v1/projects/A/commands")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let response = app
            .oneshot(
                Request::get("/api/v1/projects/A/scenarios/A/analysis")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["revision"]["scenario"], json!(["A", 0]));
        assert_eq!(body["planning_horizon"], 3);
        assert_eq!(body["candidates"], json!([]));
    }

    #[tokio::test]
    async fn analyzes_impediment_candidates_over_http() {
        let app = router();
        create_project(&app).await;
        for (revision, payload) in [
            NodePayload::Factor(Factor {
                controllable: true,
                evidence: vec![crate::domain::Evidence {
                    id: 0,
                    revision: 0,
                    summary: "Queueing observed".to_owned(),
                    source: Some("dashboard".to_owned()),
                }],
            }),
            NodePayload::Outcome(crate::domain::Outcome {
                direction: crate::domain::OutcomeDirection::Maximize,
                evidence: vec![],
            }),
        ]
        .into_iter()
        .enumerate()
        {
            app.clone()
                .oneshot(
                    Request::post("/api/v1/projects/A/commands")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::to_vec(&CommandRequest::new(
                                revision as u64,
                                GraphCommand::CreateNode(CreateNode {
                                    name: format!("node-{revision}"),
                                    title: format!("Node {revision}"),
                                    payload,
                                }),
                            ))
                            .unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
        }
        for (revision, node) in [(2, 0), (3, 1)] {
            app.clone()
                .oneshot(
                    Request::post("/api/v1/projects/A/commands")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::to_vec(&CommandRequest::new(
                                revision,
                                GraphCommand::SetNodeQuantityState(SetNodeQuantityState {
                                    node: EntityId::new(node),
                                    expected_revision: 0,
                                    quantity: QuantityDefinition::with_dimension(
                                        "state",
                                        Some(Unit::dimensionless()),
                                        None,
                                        QuantitySupport::Bounded {
                                            lower: 0.0,
                                            upper: 1.0,
                                        },
                                    )
                                    .unwrap(),
                                }),
                            ))
                            .unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
        }
        app.clone()
            .oneshot(
                Request::post("/api/v1/projects/A/commands")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&CommandRequest::new(
                            4,
                            GraphCommand::CreateEdge(CreateEdge {
                                source: EntityId::new(0),
                                destination: EntityId::new(1),
                                payload: EdgePayload::Contributes(
                                    crate::domain::CausalEffect::linear(
                                        LinearResponse {
                                            source_change: 1.0,
                                            source_unit: Unit::dimensionless(),
                                            destination_change: crate::domain::Estimate::<
                                                QuantityValue,
                                            >::new(
                                                crate::domain::EstimateId::new(0),
                                                crate::domain::Distribution::point(0.5).unwrap(),
                                            )
                                            .unwrap(),
                                            destination_unit: Unit::dimensionless(),
                                        },
                                        None,
                                        String::new(),
                                        vec!["ADR-1".to_owned()],
                                    )
                                    .unwrap(),
                                ),
                            }),
                        ))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let response = app
            .oneshot(
                Request::get("/api/v1/projects/A/analysis/impediments")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["revision"]["graph_revision"], 5);
        assert_eq!(body["topology_candidates"][0]["factor"], "A");
        assert_eq!(
            body["topology_candidates"][0]["reachable_outcomes"],
            json!(["B"])
        );
        assert_eq!(body["evidence_priority"], json!(["A"]));
        assert_eq!(
            body["topology_candidates"][0]["relationship_evidence"][0]["references"],
            json!(["ADR-1"])
        );
    }

    #[tokio::test]
    async fn sets_shows_and_removes_project_dependence() {
        let app = router();
        create_project(&app).await;
        let model = ProjectDependenceModel {
            revision: 0,
            residual_groups: vec![],
        };
        let set = CommandRequest::new(
            0,
            GraphCommand::SetProjectDependence(SetProjectDependence {
                model: model.clone(),
            }),
        );
        let created = app
            .clone()
            .oneshot(
                Request::post("/api/v1/projects/A/commands")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&set).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);
        let shown = app
            .clone()
            .oneshot(
                Request::get("/api/v1/projects/A/dependence")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response_json(shown).await,
            serde_json::to_value(&model).unwrap()
        );

        let remove = CommandRequest::new(
            1,
            GraphCommand::RemoveProjectDependence(RemoveProjectDependence {
                expected_revision: 0,
            }),
        );
        app.clone()
            .oneshot(
                Request::post("/api/v1/projects/A/commands")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&remove).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let missing = app
            .oneshot(
                Request::get("/api/v1/projects/A/dependence")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response_json(missing).await["error"]["code"],
            "dependence_not_found"
        );
    }
}
