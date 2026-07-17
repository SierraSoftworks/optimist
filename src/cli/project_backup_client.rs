use reqwest::Method;
use uuid::Uuid;

use crate::{
    domain::ProjectId,
    project::{CatalogBackup, CatalogRestore, ProjectArchive, ProjectSnapshot},
};

use super::client::{ProjectClient, decode};

impl ProjectClient {
    pub(super) async fn create_backup(&self) -> Result<CatalogBackup, human_errors::Error> {
        self.send(Method::POST, "api/v1/backups").await
    }

    pub(super) async fn list_backups(&self) -> Result<Vec<CatalogBackup>, human_errors::Error> {
        self.send(Method::GET, "api/v1/backups").await
    }

    pub(super) async fn restore_backup(
        &self,
        backup: Uuid,
        yes: bool,
    ) -> Result<CatalogRestore, human_errors::Error> {
        let response = self
            .client
            .post(self.endpoint(&format!("api/v1/backups/{backup}/restore"))?)
            .query(&[("yes", yes)])
            .send()
            .await
            .map_err(network_error)?;
        decode(response).await
    }

    pub(super) async fn create_project_snapshot(
        &self,
        project: &ProjectId,
    ) -> Result<ProjectSnapshot, human_errors::Error> {
        self.send(
            Method::POST,
            &format!("api/v1/projects/{project}/snapshots"),
        )
        .await
    }

    pub(super) async fn list_project_snapshots(
        &self,
        project: &ProjectId,
    ) -> Result<Vec<ProjectSnapshot>, human_errors::Error> {
        self.send(Method::GET, &format!("api/v1/projects/{project}/snapshots"))
            .await
    }

    pub(super) async fn get_project_snapshot(
        &self,
        project: &ProjectId,
        revision: u64,
    ) -> Result<ProjectArchive, human_errors::Error> {
        self.send(
            Method::GET,
            &format!("api/v1/projects/{project}/snapshots/{revision}"),
        )
        .await
    }

    async fn send<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
    ) -> Result<T, human_errors::Error> {
        let response = self
            .client
            .request(method, self.endpoint(path)?)
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
