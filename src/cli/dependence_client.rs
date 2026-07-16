use crate::{
    command::{
        CommandOutcome, CommandRequest, CommandResult, GraphCommand, RemoveProjectDependence,
        SetProjectDependence,
    },
    domain::{ProjectDependenceModel, ProjectId},
};

use super::client::{ProjectClient, decode};

impl ProjectClient {
    pub(super) async fn set_dependence(
        &self,
        project: &ProjectId,
        model: ProjectDependenceModel,
    ) -> Result<ProjectDependenceModel, human_errors::Error> {
        self.dependence_command(
            project,
            GraphCommand::SetProjectDependence(SetProjectDependence { model }),
        )
        .await
    }

    pub(super) async fn remove_dependence(
        &self,
        project: &ProjectId,
        expected_revision: u64,
    ) -> Result<ProjectDependenceModel, human_errors::Error> {
        self.dependence_command(
            project,
            GraphCommand::RemoveProjectDependence(RemoveProjectDependence { expected_revision }),
        )
        .await
    }

    pub(super) async fn show_dependence(
        &self,
        project: &ProjectId,
    ) -> Result<ProjectDependenceModel, human_errors::Error> {
        let response = self
            .client
            .get(self.endpoint(&format!("api/v1/projects/{project}/dependence"))?)
            .send()
            .await
            .map_err(network_error)?;
        decode(response).await
    }

    async fn dependence_command(
        &self,
        project: &ProjectId,
        command: GraphCommand,
    ) -> Result<ProjectDependenceModel, human_errors::Error> {
        let revision = self.show(project).await?.revision;
        let request = CommandRequest::new(revision, command);
        let response = self
            .client
            .post(self.endpoint(&format!("api/v1/projects/{project}/commands"))?)
            .json(&request)
            .send()
            .await
            .map_err(network_error)?;
        let result: CommandResult = decode(response).await?;
        match result.outcome {
            CommandOutcome::ProjectDependenceSet(value)
            | CommandOutcome::ProjectDependenceRemoved(value) => Ok(value),
            _ => Err(human_errors::system(
                "The Optimist server returned an unexpected dependence result.",
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
