use crate::{
    command::{
        AppendObservation, CommandOutcome, CommandRequest, CommandResult, CorrectObservation,
        GraphCommand,
    },
    domain::{EdgeId, EdgePayload, NewObservation, Observation, ProjectId},
};

use super::client::{ProjectClient, decode};

impl ProjectClient {
    pub(super) async fn append_observation(
        &self,
        project: &ProjectId,
        edge: EdgeId,
        observation: NewObservation,
    ) -> Result<Observation, human_errors::Error> {
        let request = CommandRequest::new(
            self.show(project).await?.revision,
            GraphCommand::AppendObservation(AppendObservation { edge, observation }),
        );
        let result = self.execute_observation(project, request).await?;
        match result.outcome {
            CommandOutcome::ObservationAppended { observation, .. } => Ok(observation),
            _ => Err(unexpected()),
        }
    }

    pub(super) async fn correct_observation(
        &self,
        project: &ProjectId,
        edge: EdgeId,
        observation_id: u64,
        value: f64,
    ) -> Result<Observation, human_errors::Error> {
        let request = CommandRequest::new(
            self.show(project).await?.revision,
            GraphCommand::CorrectObservation(CorrectObservation {
                edge,
                observation_id,
                value,
            }),
        );
        let result = self.execute_observation(project, request).await?;
        match result.outcome {
            CommandOutcome::ObservationCorrected { observation, .. } => Ok(observation),
            _ => Err(unexpected()),
        }
    }

    pub(super) async fn list_observations(
        &self,
        project: &ProjectId,
        edge: &EdgeId,
    ) -> Result<Vec<Observation>, human_errors::Error> {
        let edge = self.show_edge(project, edge).await?;
        match edge.payload {
            EdgePayload::Measures(measurement) => Ok(measurement.observations),
            _ => Err(human_errors::user(
                "The selected edge does not own observations.",
                &["Choose a `measures` edge returned by `optimist edge list`."],
            )),
        }
    }

    async fn execute_observation(
        &self,
        project: &ProjectId,
        request: CommandRequest,
    ) -> Result<CommandResult, human_errors::Error> {
        let response = self
            .client
            .post(self.endpoint(&format!("api/v1/projects/{project}/commands"))?)
            .json(&request)
            .send()
            .await
            .map_err(observation_network_error)?;
        decode(response).await
    }
}

fn observation_network_error(error: reqwest::Error) -> human_errors::Error {
    human_errors::wrap_system(
        error,
        "The Optimist server could not be reached.",
        &["Start `optimist server` and verify `--server-url` or `OPTIMIST_SERVER` points to it."],
    )
}

fn unexpected() -> human_errors::Error {
    human_errors::system(
        "The Optimist server returned an unexpected result for an observation command.",
        &["Confirm the CLI and server versions match, then inspect the server logs."],
    )
}

#[cfg(test)]
mod tests {
    use tokio::{net::TcpListener, task::JoinHandle};

    use crate::{
        domain::{
            EdgePayload, Factor, Measurement, MeasurementPolarity, Metric, NewObservation,
            NodePayload,
        },
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

    async fn measurement(
        client: &ProjectClient,
    ) -> (crate::domain::ProjectId, crate::domain::EdgeId) {
        let project = client.create("Delivery".to_owned()).await.unwrap();
        let metric = client
            .create_node(
                &project.id,
                "availability".to_owned(),
                "Availability".to_owned(),
                NodePayload::Metric(Metric {
                    unit: "ratio".to_owned(),
                    aggregation: None,
                }),
            )
            .await
            .unwrap();
        let factor = client
            .create_node(
                &project.id,
                "reliability".to_owned(),
                "Reliability".to_owned(),
                NodePayload::Factor(Factor {
                    current: None,
                    desired: None,
                    controllable: true,
                    evidence: vec![],
                }),
            )
            .await
            .unwrap();
        let edge = client
            .create_edge(
                &project.id,
                metric.id,
                factor.id,
                EdgePayload::Measures(Measurement {
                    polarity: MeasurementPolarity::HigherIsBetter,
                    calibration: None,
                    observations: vec![],
                }),
            )
            .await
            .unwrap();
        (project.id, edge.id())
    }

    fn observation(unit: &str) -> NewObservation {
        NewObservation {
            value: 0.9,
            unit: unit.to_owned(),
            observed_at: "2026-07-15T12:00:00Z".to_owned(),
            source: "dashboard".to_owned(),
            measurement_standard_deviation: Some(0.02),
        }
    }

    #[tokio::test]
    async fn appends_corrects_and_lists_observations_over_http() {
        let (client, server) = client().await;
        let (project, edge) = measurement(&client).await;
        let original = client
            .append_observation(&project, edge.clone(), observation("ratio"))
            .await
            .unwrap();
        let correction = client
            .correct_observation(&project, edge.clone(), original.id, 0.95)
            .await
            .unwrap();
        assert_eq!(correction.supersedes, Some(original.id));
        assert_eq!(
            client.list_observations(&project, &edge).await.unwrap(),
            vec![original, correction]
        );
        server.abort();
    }

    #[tokio::test]
    async fn returns_actionable_unit_mismatch() {
        let (client, server) = client().await;
        let (project, edge) = measurement(&client).await;
        let error = client
            .append_observation(&project, edge, observation("percent"))
            .await
            .unwrap_err();
        assert!(
            error
                .advice()
                .iter()
                .any(|item| item.contains("source metric"))
        );
        server.abort();
    }
}
