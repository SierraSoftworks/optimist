use std::{net::SocketAddr, path::PathBuf};

use axum::{Json, Router, routing::get};
use serde::Serialize;
use thiserror::Error;
use tokio::net::TcpListener;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub bind: SocketAddr,
    pub data_dir: PathBuf,
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("could not create data directory {path}")]
    DataDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not bind the HTTP listener to {address}")]
    Bind {
        address: SocketAddr,
        #[source]
        source: std::io::Error,
    },
    #[error("the HTTP server failed")]
    Serve(#[source] std::io::Error),
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    version: &'static str,
}

pub fn router() -> Router {
    Router::new().route("/api/v1/health", get(health))
}

pub async fn serve(config: ServerConfig) -> Result<(), ServerError> {
    std::fs::create_dir_all(&config.data_dir).map_err(|source| ServerError::DataDirectory {
        path: config.data_dir,
        source,
    })?;
    let listener = TcpListener::bind(config.bind)
        .await
        .map_err(|source| ServerError::Bind {
            address: config.bind,
            source,
        })?;

    axum::serve(listener, router())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(ServerError::Serve)
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use serde_json::json;
    use tower::ServiceExt;

    use super::router;

    #[tokio::test]
    async fn reports_server_health() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            json!({"status": "ok", "version": env!("CARGO_PKG_VERSION")})
        );
    }
}
