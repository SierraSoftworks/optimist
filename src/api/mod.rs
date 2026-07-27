//! The HTTP and WebSocket surface the workbench talks to.
//!
//! # Shape
//!
//! Reads return the design as it stands; the one write endpoint takes the same
//! mutations the session applies. There is no revision to send, nothing to
//! retry, and no conflict to resolve, because a mutation names one entity and
//! the last writer to touch it wins.
//!
//! # Why the feed carries mutations
//!
//! A client subscribes once and is told what changed rather than being handed a
//! new design. Sending a whole design would be simpler to implement and worse to
//! use: it would clobber whatever the recipient was midway through editing, and
//! it would cost the size of the model on every keystroke somebody else typed.
//!
//! Sending the mutation instead means a client applies exactly what the server
//! applied, to exactly the entity that changed, leaving everything it is working
//! on untouched. It also collapses the request rate. An editor that used to poll
//! for changes now opens one socket and is quiet until something happens.
//!
//! # Why the socket opens with a snapshot
//!
//! Fetching a design and then subscribing would drop anything that changed in
//! between. Subscribing and then fetching would deliver changes the fetch
//! already contains. The socket therefore sends the design as its first message
//! and streams changes after it, which has no gap to reason about and costs one
//! round trip rather than two.

mod analysis;
mod designs;
mod error;
mod feed;

use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use axum::{Json, Router, routing::get};
use serde::Serialize;
use tokio::net::TcpListener;

pub use error::ApiError;

use crate::session::Workspace;

/// How often loaded designs are checked for edits that have settled.
const SWEEP_INTERVAL: Duration = Duration::from_millis(100);

/// What the process needs in order to serve a workspace.
#[derive(Clone, Debug)]
pub struct ApiConfig {
    /// Address on which requests are accepted.
    pub bind: SocketAddr,
    /// Directory holding the designs to serve.
    pub designs: PathBuf,
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    version: &'static str,
    designs: usize,
    unsaved: usize,
}

/// Builds a router over a workspace.
///
/// ```
/// use std::sync::Arc;
/// use optimist::{api::router, session::Workspace};
///
/// let app = router(Arc::new(Workspace::new("designs")));
/// # let _ = app;
/// ```
pub fn router(workspace: Arc<Workspace>) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .merge(designs::router())
        .merge(analysis::router())
        .merge(feed::router())
        .with_state(workspace)
}

async fn health(
    axum::extract::State(workspace): axum::extract::State<Arc<Workspace>>,
) -> Json<Health> {
    let loaded = workspace.loaded();
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        designs: loaded.len(),
        unsaved: loaded
            .iter()
            .filter(|(_, session)| session.pending())
            .count(),
    })
}

/// Serves a workspace until the process is asked to stop.
///
/// Designs that have settled are written by a background sweep, and anything
/// still unsaved is written on the way out, so an edit is never lost to a
/// shutdown that arrived during the quiet period.
pub async fn serve(config: ApiConfig) -> Result<(), ApiError> {
    let workspace = Arc::new(Workspace::new(config.designs));
    let listener = TcpListener::bind(config.bind)
        .await
        .map_err(|source| ApiError::Bind {
            address: config.bind,
            source,
        })?;

    let sweeping = Arc::clone(&workspace);
    let sweep = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            ticker.tick().await;
            if let Err(error) = sweeping.persist_due() {
                eprintln!("a design could not be written: {error}");
            }
        }
    });

    let served = Arc::clone(&workspace);
    let result = axum::serve(listener, router(workspace))
        .with_graceful_shutdown(shutdown())
        .await
        .map_err(|source| ApiError::Serve { source });

    sweep.abort();
    if let Err(error) = served.persist_all() {
        eprintln!("unsaved designs could not be written on shutdown: {error}");
    }
    result
}

async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}
