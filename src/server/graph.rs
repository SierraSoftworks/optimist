use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};

use crate::{
    command::{CommandRequest, CommandResult},
    domain::{Edge, EdgeId, EntityId, Node, ProjectId, Scenario, ScenarioId},
    project::ProjectError,
    store::RepositoryError,
};

use super::{AppState, api_error::ApiError};

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/projects/{project}/commands", post(execute_command))
        .route("/api/v1/projects/{project}/nodes", get(list_nodes))
        .route("/api/v1/projects/{project}/nodes/{entity}", get(show_node))
        .route("/api/v1/projects/{project}/edges", get(list_edges))
        .route("/api/v1/projects/{project}/edges/{edge}", get(show_edge))
        .route("/api/v1/projects/{project}/scenarios", get(list_scenarios))
        .route(
            "/api/v1/projects/{project}/scenarios/{scenario}",
            get(show_scenario),
        )
}

async fn execute_command(
    State(state): State<AppState>,
    Path(project): Path<ProjectId>,
    Json(request): Json<CommandRequest>,
) -> Result<(StatusCode, Json<CommandResult>), ApiError> {
    let result = state.catalog.write().await.execute(&project, request)?;
    Ok((StatusCode::CREATED, Json(result)))
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

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use crate::{
        command::{CommandRequest, CreateNode, CreateScenario, GraphCommand},
        domain::{Factor, MonteCarloConfig, NodePayload, ScenarioDraft},
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
                    current: None,
                    desired: None,
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
}
