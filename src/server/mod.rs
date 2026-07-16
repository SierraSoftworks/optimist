mod api_error;
mod error;
mod estimate_error_response;
mod formula_error_response;
mod formulas;
mod graph;
mod project_error_response;
mod projects;
mod repository_error_response;
mod state;

use std::{net::SocketAddr, path::PathBuf};

use axum::{Json, Router, routing::get};
use serde::Serialize;
use tokio::net::TcpListener;

pub use error::ServerError;
use state::AppState;

use crate::project::ProjectCatalog;

/// Configuration required to run the Optimist HTTP process.
///
/// ```
/// use optimist::server::ServerConfig;
/// let config = ServerConfig {
///     bind: "127.0.0.1:3000".parse()?,
///     data_dir: ".optimist".into(),
/// };
/// # Ok::<(), std::net::AddrParseError>(())
/// ```
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// TCP address on which API and frontend requests are accepted.
    pub bind: SocketAddr,
    /// Root directory reserved for project catalogs and per-project databases.
    pub data_dir: PathBuf,
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    version: &'static str,
}

/// Builds a fresh application router with an empty process-local project catalog.
///
/// Use [`router_with_catalog`] in tests or embedding scenarios that need a prepared
/// catalog. Production persistence will replace the empty catalog during startup.
pub fn router() -> Router {
    router_with_catalog(ProjectCatalog::new())
}

/// Builds an application router around a caller-provided project catalog.
///
/// ```
/// use optimist::{project::ProjectCatalog, server::router_with_catalog};
/// let app = router_with_catalog(ProjectCatalog::new());
/// # let _ = app;
/// ```
pub fn router_with_catalog(catalog: ProjectCatalog) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .merge(projects::router())
        .merge(graph::router())
        .with_state(AppState::new(catalog))
}

/// Creates the data root, binds the listener, and serves until shutdown or failure.
///
/// The function installs graceful Ctrl-C shutdown and returns startup/serve failures
/// to the CLI for one-time pretty rendering.
///
/// ```no_run
/// use optimist::server::{ServerConfig, serve};
///
/// # async fn example() -> Result<(), optimist::server::ServerError> {
/// serve(ServerConfig {
///     bind: "127.0.0.1:3000".parse().unwrap(),
///     data_dir: ".optimist".into(),
/// }).await
/// # }
/// ```
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
        body::Body,
        http::{Request, StatusCode},
    };
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
    }
}
