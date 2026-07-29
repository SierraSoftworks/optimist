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
mod cache;
mod designs;
mod error;
mod feed;
mod solving;
mod web;

use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    extract::FromRef,
    routing::{any, get},
};
use serde::Serialize;
use tokio::net::TcpListener;

pub use error::ApiError;

use crate::session::Workspace;

/// How often loaded designs are checked for edits that have settled.
const SWEEP_INTERVAL: Duration = Duration::from_millis(100);

/// Everything a request may need beyond what it carries itself.
///
/// The workspace is the authority on what a design says; the caches are a
/// performance artifact over it. They are separate fields rather than one type
/// because an answer's lifetime is decided by how often somebody asks for it,
/// and a design's by whether anybody has it open.
///
/// The board and the in-flight registries are about the answers that do not
/// exist yet: which solves are running, so that watchers can be told, and which
/// have already been started, so that asking again joins one rather than
/// starting another.
#[derive(Clone)]
pub(super) struct ApiState {
    workspace: Arc<Workspace>,
    analyses: Arc<cache::Cache<analysis::Analysis>>,
    comparisons: Arc<cache::Cache<crate::system::Comparison>>,
    solving: Arc<solving::Board>,
    pending: Arc<solving::InFlight<analysis::Analysis>>,
    weighing: Arc<solving::InFlight<crate::system::Comparison>>,
}

impl ApiState {
    fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            workspace,
            analyses: Arc::new(cache::Cache::new()),
            comparisons: Arc::new(cache::Cache::new()),
            solving: Arc::new(solving::Board::default()),
            pending: Arc::new(solving::InFlight::default()),
            weighing: Arc::new(solving::InFlight::default()),
        }
    }
}

/// Lets a handler that only reads designs keep asking for the workspace alone.
impl FromRef<ApiState> for Arc<Workspace> {
    fn from_ref(state: &ApiState) -> Self {
        Arc::clone(&state.workspace)
    }
}

/// What the process needs in order to serve a workspace.
#[derive(Clone, Debug)]
pub struct ApiConfig {
    /// Address on which requests are accepted.
    pub bind: SocketAddr,
    /// Directory holding the designs to serve.
    pub designs: PathBuf,
    /// A frontend build to serve, overriding whatever the binary would use.
    pub web_root: Option<PathBuf>,
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
    routes(workspace, None)
}

/// Builds a router that also serves a frontend build.
///
/// The workbench is mounted as a fallback, so it is reached only by requests no
/// API route claimed. That ordering is what keeps an unknown `/api` path a JSON
/// error rather than a page of HTML.
pub fn routes(workspace: Arc<Workspace>, web_root: Option<PathBuf>) -> Router {
    let api = Router::new()
        .route("/api/v1/health", get(health))
        .merge(designs::router())
        .merge(analysis::router())
        .merge(feed::router())
        // Claims every remaining API path so that the workbench fallback below
        // never sees one. A mistyped endpoint answered with a page of HTML
        // would be read by a client as a malformed success rather than as the
        // 404 it is, and owning the refusal here means that holds whether or
        // not a frontend is being served at all.
        .route("/api/{*rest}", any(unknown_endpoint))
        .with_state(ApiState::new(workspace));
    web::attach(api, web::Assets::new(web_root))
}

async fn unknown_endpoint() -> impl axum::response::IntoResponse {
    (
        axum::http::StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "message": "No such endpoint.",
            "advice": "Check the path against the API reference.",
        })),
    )
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
    let result = axum::serve(listener, routes(workspace, config.web_root))
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
