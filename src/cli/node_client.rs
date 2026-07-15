use crate::{
    command::{
        CommandOutcome, CommandRequest, CommandResult, CreateNode, DeleteNode, GraphCommand,
    },
    domain::{EntityId, Node, NodePayload, ProjectId},
};

use super::client::{ProjectClient, decode};

impl ProjectClient {
    pub(super) async fn create_node(
        &self,
        project: &ProjectId,
        name: String,
        title: String,
        payload: NodePayload,
    ) -> Result<Node, human_errors::Error> {
        let revision = self.show(project).await?.revision;
        let request = CommandRequest::new(
            revision,
            GraphCommand::CreateNode(CreateNode {
                name,
                title,
                payload,
            }),
        );
        let response = self
            .client
            .post(self.endpoint(&format!("api/v1/projects/{project}/commands"))?)
            .json(&request)
            .send()
            .await
            .map_err(|error| {
                human_errors::wrap_system(
                    error,
                    "The Optimist server could not be reached.",
                    &["Start `optimist server` and verify `--server-url` or `OPTIMIST_SERVER` points to it."],
                )
            })?;
        let result: CommandResult = decode(response).await?;
        match result.outcome {
            CommandOutcome::NodeCreated(node) => Ok(node),
            _ => Err(human_errors::system(
                "The Optimist server returned an unexpected result for a node command.",
                &["Confirm the CLI and server versions match, then inspect the server logs."],
            )),
        }
    }

    pub(super) async fn list_nodes(
        &self,
        project: &ProjectId,
    ) -> Result<Vec<Node>, human_errors::Error> {
        let response = self
            .client
            .get(self.endpoint(&format!("api/v1/projects/{project}/nodes"))?)
            .send()
            .await
            .map_err(node_network_error)?;
        decode(response).await
    }

    pub(super) async fn delete_node(
        &self,
        project: &ProjectId,
        entity: EntityId,
    ) -> Result<Node, human_errors::Error> {
        let revision = self.show(project).await?.revision;
        let request = CommandRequest::new(
            revision,
            GraphCommand::DeleteNode(DeleteNode { id: entity }),
        );
        let response = self
            .client
            .post(self.endpoint(&format!("api/v1/projects/{project}/commands"))?)
            .json(&request)
            .send()
            .await
            .map_err(node_network_error)?;
        let result: CommandResult = decode(response).await?;
        match result.outcome {
            CommandOutcome::NodeDeleted(node) => Ok(node),
            _ => Err(human_errors::system(
                "The Optimist server returned an unexpected result for a node command.",
                &["Confirm the CLI and server versions match, then inspect the server logs."],
            )),
        }
    }

    pub(super) async fn show_node(
        &self,
        project: &ProjectId,
        entity: EntityId,
    ) -> Result<Node, human_errors::Error> {
        let response = self
            .client
            .get(self.endpoint(&format!("api/v1/projects/{project}/nodes/{entity}"))?)
            .send()
            .await
            .map_err(node_network_error)?;
        decode(response).await
    }
}

fn node_network_error(error: reqwest::Error) -> human_errors::Error {
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
        domain::{Factor, NodePayload, ProjectId},
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

    #[tokio::test]
    async fn creates_and_reads_nodes_over_http() {
        let (client, server) = client().await;
        let project = client.create("Delivery".to_owned()).await.unwrap();
        let node = client
            .create_node(
                &project.id,
                "github".to_owned(),
                "GitHub".to_owned(),
                NodePayload::Factor(Factor {
                    current: None,
                    desired: None,
                    controllable: false,
                    evidence: vec![],
                }),
            )
            .await
            .unwrap();
        assert_eq!(
            client.list_nodes(&project.id).await.unwrap(),
            vec![node.clone()]
        );
        assert_eq!(client.show_node(&project.id, node.id).await.unwrap(), node);
        server.abort();
    }

    #[tokio::test]
    async fn requires_an_existing_project() {
        let (client, server) = client().await;
        let error = client
            .list_nodes(&ProjectId::new("A").unwrap())
            .await
            .unwrap_err();
        assert!(
            error
                .advice()
                .iter()
                .any(|item| item.contains("project list"))
        );
        server.abort();
    }
}
