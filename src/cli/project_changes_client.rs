use crate::{command::ChangeSetReplay, domain::ProjectId};

use super::client::{ProjectClient, decode};

impl ProjectClient {
    pub(super) async fn replay_changes(
        &self,
        project: &ProjectId,
        after: u64,
    ) -> Result<ChangeSetReplay, human_errors::Error> {
        let response = self
            .client
            .get(self.endpoint(&format!("api/v1/projects/{project}/changes"))?)
            .query(&[("after", after)])
            .send()
            .await
            .map_err(network_error)?;
        decode(response).await
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
        domain::{Factor, NodePayload},
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
    async fn replays_committed_changes_over_http() {
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
        let replay = client.replay_changes(&project.id, 0).await.unwrap();
        assert_eq!(replay.current_revision, 1);
        assert_eq!(replay.changes.len(), 1);
        assert_eq!(replay.changes[0].project_revision, 1);
        assert_eq!(replay.changes[0].graph_revision, 1);

        let error = client.replay_changes(&project.id, 2).await.unwrap_err();
        assert!(
            error
                .advice()
                .iter()
                .any(|item| item.contains("current revision"))
        );
        server.abort();
    }
}
