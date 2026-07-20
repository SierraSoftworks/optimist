use uuid::Uuid;

use crate::{
    command::{CommandBatchRequest, CommandBatchResult, CompensatingUndoRequest, GraphCommand},
    domain::ProjectId,
};

use super::client::{ProjectClient, decode};

impl ProjectClient {
    pub(super) async fn execute_batch(
        &self,
        project: &ProjectId,
        request_id: Uuid,
        expected_revision: u64,
        commands: Vec<GraphCommand>,
    ) -> Result<CommandBatchResult, human_errors::Error> {
        let response = self
            .client
            .post(self.endpoint(&format!("api/v1/projects/{project}/command-batches"))?)
            .json(&CommandBatchRequest {
                request_id,
                expected_revision,
                commands,
            })
            .send()
            .await
            .map_err(network_error)?;
        decode(response).await
    }

    pub(super) async fn undo_batch(
        &self,
        project: &ProjectId,
        batch: Uuid,
        request_id: Uuid,
        expected_revision: u64,
        commands: Vec<GraphCommand>,
    ) -> Result<CommandBatchResult, human_errors::Error> {
        let response = self
            .client
            .post(self.endpoint(&format!(
                "api/v1/projects/{project}/command-batches/{batch}/undo"
            ))?)
            .json(&CompensatingUndoRequest {
                request_id,
                expected_revision,
                commands,
            })
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
