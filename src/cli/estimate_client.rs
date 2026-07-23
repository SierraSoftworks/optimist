use crate::{
    command::{
        CommandOutcome, CommandRequest, CommandResult, GraphCommand, RemoveEstimate, SetEstimate,
    },
    domain::{
        Distribution, EstimateAddress, EstimateSlot, EstimateUncertainty, PrimitiveEstimate,
        ProjectId,
    },
};

use super::client::{ProjectClient, decode};

impl ProjectClient {
    pub(super) async fn set_estimate(
        &self,
        project: &ProjectId,
        address: EstimateAddress,
        slot: EstimateSlot,
        distribution: Distribution,
        provenance: Vec<String>,
        uncertainty: EstimateUncertainty,
    ) -> Result<PrimitiveEstimate, human_errors::Error> {
        self.estimate_command(
            project,
            GraphCommand::SetEstimate(SetEstimate {
                address,
                slot,
                distribution,
                provenance,
                uncertainty,
            }),
        )
        .await
    }

    pub(super) async fn remove_estimate(
        &self,
        project: &ProjectId,
        address: EstimateAddress,
    ) -> Result<PrimitiveEstimate, human_errors::Error> {
        self.estimate_command(
            project,
            GraphCommand::RemoveEstimate(RemoveEstimate { address }),
        )
        .await
    }

    pub(super) async fn show_estimate(
        &self,
        project: &ProjectId,
        address: &EstimateAddress,
    ) -> Result<PrimitiveEstimate, human_errors::Error> {
        let response = self
            .client
            .get(self.endpoint(&format!("api/v1/projects/{project}/estimates"))?)
            .query(&[("address", address.to_string())])
            .send()
            .await
            .map_err(network_error)?;
        decode(response).await
    }

    async fn estimate_command(
        &self,
        project: &ProjectId,
        command: GraphCommand,
    ) -> Result<PrimitiveEstimate, human_errors::Error> {
        let revision = self.show(project).await?.revision;
        let response = self
            .client
            .post(self.endpoint(&format!("api/v1/projects/{project}/commands"))?)
            .json(&CommandRequest::new(revision, command))
            .send()
            .await
            .map_err(network_error)?;
        let result: CommandResult = decode(response).await?;
        match result.outcome {
            CommandOutcome::EstimateSet(value) | CommandOutcome::EstimateRemoved(value) => {
                Ok(value)
            }
            _ => Err(human_errors::system(
                "The Optimist server returned an unexpected result for an estimate command.",
                &["Confirm the CLI and server versions match, then inspect the server logs."],
            )),
        }
    }
}

fn network_error(error: reqwest::Error) -> human_errors::Error {
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
        domain::{
            Distribution, EntityId, EstimateAddress, EstimateId, EstimateOwner, EstimateSlot,
            Factor, NodePayload,
        },
        server,
    };

    use super::ProjectClient;

    async fn client() -> (ProjectClient, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, server::router()).await.unwrap();
        });
        (
            ProjectClient::new(&format!("http://{address}")).unwrap(),
            task,
        )
    }

    #[tokio::test]
    async fn performs_primitive_estimate_lifecycle_over_http() {
        let (client, server) = client().await;
        let project = client.create("Delivery".to_owned()).await.unwrap();
        client
            .create_node(
                &project.id,
                "flow".to_owned(),
                "Flow".to_owned(),
                NodePayload::Factor(Factor {
                    current: None,
                    desired: None,
                    controllable: true,
                    evidence: vec![],
                }),
            )
            .await
            .unwrap();
        let address = EstimateAddress::new(
            project.id.clone(),
            EstimateOwner::Node(EntityId::new(0)),
            EstimateId::new(0),
        );
        let created = client
            .set_estimate(
                &project.id,
                address.clone(),
                EstimateSlot::Current,
                Distribution::beta(2.0, 3.0).unwrap(),
                vec!["elicitation".to_owned()],
                crate::domain::EstimateUncertainty::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            client.show_estimate(&project.id, &address).await.unwrap(),
            created
        );
        assert_eq!(
            client
                .remove_estimate(&project.id, address.clone())
                .await
                .unwrap(),
            created
        );
        let error = client
            .show_estimate(&project.id, &address)
            .await
            .unwrap_err();
        assert!(error.advice().iter().any(|item| item.contains("embedded")));
        server.abort();
    }
}
