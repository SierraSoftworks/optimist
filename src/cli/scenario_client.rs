use crate::{
    command::{
        CommandOutcome, CommandRequest, CommandResult, CreateScenario, DeleteScenario,
        GraphCommand, UpdateScenario,
    },
    domain::{ProjectId, Scenario, ScenarioDraft, ScenarioId},
};

use super::client::{ProjectClient, decode};

impl ProjectClient {
    pub(super) async fn create_scenario(
        &self,
        project: &ProjectId,
        scenario: ScenarioDraft,
    ) -> Result<Scenario, human_errors::Error> {
        self.scenario_command(
            project,
            GraphCommand::CreateScenario(CreateScenario { scenario }),
        )
        .await
    }

    pub(super) async fn update_scenario(
        &self,
        project: &ProjectId,
        id: ScenarioId,
        expected_revision: u64,
        scenario: ScenarioDraft,
    ) -> Result<Scenario, human_errors::Error> {
        self.scenario_command(
            project,
            GraphCommand::UpdateScenario(UpdateScenario {
                id,
                expected_revision,
                scenario,
            }),
        )
        .await
    }

    pub(super) async fn delete_scenario(
        &self,
        project: &ProjectId,
        id: ScenarioId,
        expected_revision: u64,
    ) -> Result<Scenario, human_errors::Error> {
        self.scenario_command(
            project,
            GraphCommand::DeleteScenario(DeleteScenario {
                id,
                expected_revision,
            }),
        )
        .await
    }

    pub(super) async fn list_scenarios(
        &self,
        project: &ProjectId,
    ) -> Result<Vec<Scenario>, human_errors::Error> {
        let response = self
            .client
            .get(self.endpoint(&format!("api/v1/projects/{project}/scenarios"))?)
            .send()
            .await
            .map_err(scenario_network_error)?;
        decode(response).await
    }

    pub(super) async fn show_scenario(
        &self,
        project: &ProjectId,
        id: ScenarioId,
    ) -> Result<Scenario, human_errors::Error> {
        let response = self
            .client
            .get(self.endpoint(&format!("api/v1/projects/{project}/scenarios/{id}"))?)
            .send()
            .await
            .map_err(scenario_network_error)?;
        decode(response).await
    }

    async fn scenario_command(
        &self,
        project: &ProjectId,
        command: GraphCommand,
    ) -> Result<Scenario, human_errors::Error> {
        let revision = self.show(project).await?.revision;
        let request = CommandRequest::new(revision, command);
        let response = self
            .client
            .post(self.endpoint(&format!("api/v1/projects/{project}/commands"))?)
            .json(&request)
            .send()
            .await
            .map_err(scenario_network_error)?;
        let result: CommandResult = decode(response).await?;
        match result.outcome {
            CommandOutcome::ScenarioCreated(value)
            | CommandOutcome::ScenarioUpdated(value)
            | CommandOutcome::ScenarioDeleted(value) => Ok(value),
            _ => Err(human_errors::system(
                "The Optimist server returned an unexpected result for a scenario command.",
                &["Confirm the CLI and server versions match, then inspect the server logs."],
            )),
        }
    }
}

fn scenario_network_error(error: reqwest::Error) -> human_errors::Error {
    human_errors::wrap_system(
        error,
        "The Optimist server could not be reached.",
        &["Start `optimist server` and verify `--server-url` or `OPTIMIST_SERVER` points to it."],
    )
}

#[cfg(test)]
mod tests {
    use tokio::{net::TcpListener, task::JoinHandle};

    use crate::{
        domain::{MonteCarloConfig, ScenarioDraft},
        server,
    };

    use super::ProjectClient;

    async fn client() -> (ProjectClient, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task =
            tokio::spawn(async move { axum::serve(listener, server::router()).await.unwrap() });
        (
            ProjectClient::new(&format!("http://{address}")).unwrap(),
            task,
        )
    }

    fn draft(title: &str) -> ScenarioDraft {
        ScenarioDraft {
            name: "delivery".to_owned(),
            title: title.to_owned(),
            rationale: String::new(),
            objectives: vec![],
            planning_horizon: 1,
            budgets: vec![],
            candidate_interventions: vec![],
            monte_carlo: MonteCarloConfig::new(1, 2, 10, 0.1, 0.1).unwrap(),
            scalar_preferences: None,
        }
    }

    #[tokio::test]
    async fn performs_scenario_lifecycle_over_http() {
        let (client, server) = client().await;
        let project = client.create("Delivery".to_owned()).await.unwrap();
        let created = client
            .create_scenario(&project.id, draft("Delivery"))
            .await
            .unwrap();
        assert_eq!(
            client.list_scenarios(&project.id).await.unwrap(),
            vec![created.clone()]
        );
        assert_eq!(
            client.show_scenario(&project.id, created.id).await.unwrap(),
            created
        );

        let updated = client
            .update_scenario(&project.id, created.id, 0, draft("Reliable delivery"))
            .await
            .unwrap();
        assert_eq!(updated.revision, 1);
        let error = client
            .delete_scenario(&project.id, updated.id, 0)
            .await
            .unwrap_err();
        assert!(
            error
                .advice()
                .iter()
                .any(|item| item.contains("scenario show"))
        );
        client
            .delete_scenario(&project.id, updated.id, 1)
            .await
            .unwrap();
        assert!(client.list_scenarios(&project.id).await.unwrap().is_empty());
        server.abort();
    }
}
