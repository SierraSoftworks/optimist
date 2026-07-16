use crate::{
    domain::ProjectId,
    project::{Project, ProjectArchive},
};

use super::client::{ProjectClient, decode};

impl ProjectClient {
    pub(super) async fn export_archive(
        &self,
        project: &ProjectId,
    ) -> Result<ProjectArchive, human_errors::Error> {
        let response = self
            .client
            .get(self.endpoint(&format!("api/v1/projects/{project}/archive"))?)
            .send()
            .await
            .map_err(network_error)?;
        decode(response).await
    }

    pub(super) async fn import_archive(
        &self,
        archive: &ProjectArchive,
        replace: bool,
        yes: bool,
    ) -> Result<Project, human_errors::Error> {
        let response = self
            .client
            .post(self.endpoint("api/v1/project-archives")?)
            .query(&[("replace", replace), ("yes", yes)])
            .json(archive)
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
