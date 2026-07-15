use reqwest::{Client, Method, Response, Url};
use serde::Deserialize;

use crate::{
    domain::ProjectId,
    project::{CreateProject, Project},
};

pub(super) struct ProjectClient {
    base_url: Url,
    client: Client,
}

impl ProjectClient {
    pub(super) fn new(server_url: &str) -> Result<Self, human_errors::Error> {
        let mut base_url = Url::parse(server_url).map_err(|error| {
            human_errors::wrap_user(
                error,
                "The Optimist server URL is invalid.",
                &["Provide an absolute URL such as `http://127.0.0.1:3000` with `--server-url` or `OPTIMIST_SERVER`."],
            )
        })?;
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }
        Ok(Self {
            base_url,
            client: Client::new(),
        })
    }

    pub(super) async fn create(&self, name: String) -> Result<Project, human_errors::Error> {
        let response = self
            .client
            .post(self.endpoint("api/v1/projects")?)
            .json(&CreateProject { name })
            .send()
            .await
            .map_err(network_error)?;
        decode(response).await
    }

    pub(super) async fn list(&self) -> Result<Vec<Project>, human_errors::Error> {
        let response = self.request(Method::GET, "api/v1/projects").await?;
        decode(response).await
    }

    pub(super) async fn show(&self, project: &ProjectId) -> Result<Project, human_errors::Error> {
        let response = self
            .request(Method::GET, &format!("api/v1/projects/{project}"))
            .await?;
        decode(response).await
    }

    pub(super) async fn delete(&self, project: &ProjectId) -> Result<Project, human_errors::Error> {
        let response = self
            .request(Method::DELETE, &format!("api/v1/projects/{project}"))
            .await?;
        decode(response).await
    }

    async fn request(&self, method: Method, path: &str) -> Result<Response, human_errors::Error> {
        self.client
            .request(method, self.endpoint(path)?)
            .send()
            .await
            .map_err(network_error)
    }

    fn endpoint(&self, path: &str) -> Result<Url, human_errors::Error> {
        self.base_url.join(path).map_err(|error| {
            human_errors::wrap_user(
                error,
                "Optimist could not construct the requested server URL.",
                &["Check `--server-url` for an absolute HTTP or HTTPS URL without invalid path segments."],
            )
        })
    }
}

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Deserialize)]
struct ErrorBody {
    code: String,
    message: String,
}

async fn decode<T: serde::de::DeserializeOwned>(
    response: Response,
) -> Result<T, human_errors::Error> {
    if response.status().is_success() {
        return response.json().await.map_err(|error| {
            human_errors::wrap_system(
                error,
                "The Optimist server returned an unreadable success response.",
                &["Confirm the CLI and server versions match, then retry the command."],
            )
        });
    }

    let status = response.status();
    let error = response.json::<ErrorEnvelope>().await.map_err(|cause| {
        human_errors::wrap_system(
            cause,
            "The Optimist server returned an unreadable error response.",
            &["Confirm the CLI and server versions match, then inspect the server logs."],
        )
    })?;
    Err(human_errors::user(
        error.error.message,
        advice(error.error.code.as_str(), status),
    ))
}

fn network_error(error: reqwest::Error) -> human_errors::Error {
    human_errors::wrap_system(
        error,
        "The Optimist server could not be reached.",
        &["Start `optimist server` and verify `--server-url` or `OPTIMIST_SERVER` points to it."],
    )
}

fn advice(code: &str, status: reqwest::StatusCode) -> &'static [&'static str] {
    match code {
        "invalid_project_name" => &["Provide a non-empty project name."],
        "project_name_conflict" => &["Choose a project name which is not already in use."],
        "project_not_found" => {
            &["Run `optimist project list` and retry with a returned project ID."]
        }
        _ if status.is_server_error() => {
            &["Retry the request and inspect server logs if it persists."]
        }
        _ => &["Check the command arguments and retry the request."],
    }
}

#[cfg(test)]
mod tests {
    use tokio::{net::TcpListener, task::JoinHandle};

    use crate::{domain::ProjectId, server};

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
    async fn performs_project_lifecycle_over_http() {
        let (client, server) = client().await;
        let project = client.create("Delivery".to_owned()).await.unwrap();
        assert_eq!(client.list().await.unwrap(), vec![project.clone()]);
        assert_eq!(client.show(&project.id).await.unwrap(), project);
        client.delete(&project.id).await.unwrap();
        assert!(client.list().await.unwrap().is_empty());
        server.abort();
    }

    #[tokio::test]
    async fn preserves_actionable_api_errors() {
        let (client, server) = client().await;
        let error = client
            .show(&ProjectId::new("A").unwrap())
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
