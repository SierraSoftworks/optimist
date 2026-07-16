use crate::{
    command::{CommandOutcome, CommandRequest, CommandResult, GraphCommand, UpdateNodeMetadata},
    domain::{EntityId, Node, ProjectId},
};

use super::client::{ProjectClient, decode};

impl ProjectClient {
    pub(super) async fn update_node_metadata(
        &self,
        project: &ProjectId,
        entity: EntityId,
        title: String,
        description: String,
        metadata: std::collections::BTreeMap<String, serde_json::Value>,
    ) -> Result<Node, human_errors::Error> {
        let node = self.show_node(project, entity).await?;
        let project_revision = self.show(project).await?.revision;
        let response = self
            .client
            .post(self.endpoint(&format!("api/v1/projects/{project}/commands"))?)
            .json(&CommandRequest::new(
                project_revision,
                GraphCommand::UpdateNodeMetadata(UpdateNodeMetadata {
                    id: entity,
                    expected_revision: node.revision,
                    title,
                    description,
                    metadata,
                }),
            ))
            .send()
            .await
            .map_err(network_error)?;
        let result: CommandResult = decode(response).await?;
        match result.outcome {
            CommandOutcome::NodeMetadataUpdated(node) => Ok(node),
            _ => Err(human_errors::system(
                "The Optimist server returned an unexpected result for a node command.",
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
