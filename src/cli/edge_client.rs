use crate::{
    command::{
        CommandOutcome, CommandRequest, CommandResult, CreateEdge, DeleteEdge, GraphCommand,
    },
    domain::{Edge, EdgeId, EdgePayload, ProjectId},
};

use super::client::{ProjectClient, decode};

impl ProjectClient {
    pub(super) async fn create_edge(
        &self,
        project: &ProjectId,
        source: crate::domain::EntityId,
        destination: crate::domain::EntityId,
        payload: EdgePayload,
    ) -> Result<Edge, human_errors::Error> {
        let revision = self.show(project).await?.revision;
        let request = CommandRequest::new(
            revision,
            GraphCommand::CreateEdge(CreateEdge {
                source,
                destination,
                payload,
            }),
        );
        let response = self
            .client
            .post(self.endpoint(&format!("api/v1/projects/{project}/commands"))?)
            .json(&request)
            .send()
            .await
            .map_err(edge_network_error)?;
        let result: CommandResult = decode(response).await?;
        match result.outcome {
            CommandOutcome::EdgeCreated(edge) => Ok(edge),
            _ => Err(human_errors::system(
                "The Optimist server returned an unexpected result for an edge command.",
                &["Confirm the CLI and server versions match, then inspect the server logs."],
            )),
        }
    }

    pub(super) async fn list_edges(
        &self,
        project: &ProjectId,
    ) -> Result<Vec<Edge>, human_errors::Error> {
        let response = self
            .client
            .get(self.endpoint(&format!("api/v1/projects/{project}/edges"))?)
            .send()
            .await
            .map_err(edge_network_error)?;
        decode(response).await
    }

    pub(super) async fn delete_edge(
        &self,
        project: &ProjectId,
        edge: EdgeId,
    ) -> Result<Edge, human_errors::Error> {
        let revision = self.show(project).await?.revision;
        let request =
            CommandRequest::new(revision, GraphCommand::DeleteEdge(DeleteEdge { id: edge }));
        let response = self
            .client
            .post(self.endpoint(&format!("api/v1/projects/{project}/commands"))?)
            .json(&request)
            .send()
            .await
            .map_err(edge_network_error)?;
        let result: CommandResult = decode(response).await?;
        match result.outcome {
            CommandOutcome::EdgeDeleted(edge) => Ok(edge),
            _ => Err(human_errors::system(
                "The Optimist server returned an unexpected result for an edge command.",
                &["Confirm the CLI and server versions match, then inspect the server logs."],
            )),
        }
    }

    pub(super) async fn show_edge(
        &self,
        project: &ProjectId,
        edge: &EdgeId,
    ) -> Result<Edge, human_errors::Error> {
        let response = self
            .client
            .get(self.endpoint(&format!("api/v1/projects/{project}/edges/{edge}"))?)
            .send()
            .await
            .map_err(edge_network_error)?;
        decode(response).await
    }
}

fn edge_network_error(error: reqwest::Error) -> human_errors::Error {
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
        domain::{EdgePayload, Factor, NodePayload, Requirement},
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

    fn factor() -> NodePayload {
        NodePayload::Factor(Factor {
            current: None,
            desired: None,
            controllable: false,
            evidence: vec![],
        })
    }

    #[tokio::test]
    async fn creates_reads_and_deletes_graph_items_over_http() {
        let (client, server) = client().await;
        let project = client.create("Delivery".to_owned()).await.unwrap();
        let actions = client
            .create_node(
                &project.id,
                "actions".to_owned(),
                "Actions".to_owned(),
                factor(),
            )
            .await
            .unwrap();
        let github = client
            .create_node(
                &project.id,
                "github".to_owned(),
                "GitHub".to_owned(),
                factor(),
            )
            .await
            .unwrap();
        let edge = client
            .create_edge(
                &project.id,
                actions.id,
                github.id,
                EdgePayload::Requires(Requirement {
                    hard: true,
                    satisfaction_threshold: None,
                }),
            )
            .await
            .unwrap();

        assert_eq!(
            client.list_edges(&project.id).await.unwrap(),
            vec![edge.clone()]
        );
        assert_eq!(
            client.show_edge(&project.id, &edge.id()).await.unwrap(),
            edge
        );

        let blocked = client
            .delete_node(&project.id, actions.id)
            .await
            .unwrap_err();
        assert!(
            blocked
                .advice()
                .iter()
                .any(|item| item.contains("edge list"))
        );

        let deleted_edge = client.delete_edge(&project.id, edge.id()).await.unwrap();
        assert_eq!(deleted_edge, edge);
        assert!(client.list_edges(&project.id).await.unwrap().is_empty());

        let deleted_node = client.delete_node(&project.id, actions.id).await.unwrap();
        assert_eq!(deleted_node, actions);
        assert_eq!(client.list_nodes(&project.id).await.unwrap(), vec![github]);
        server.abort();
    }
}
