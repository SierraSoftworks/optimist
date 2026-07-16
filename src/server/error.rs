use std::{net::SocketAddr, path::PathBuf};

use thiserror::Error;

/// Process-level failures which prevent the HTTP server from starting or continuing.
///
/// The CLI wraps these errors with `human_errors` advice while preserving the I/O
/// source chain for diagnostics.
#[derive(Debug, Error)]
pub enum ServerError {
    /// The configured project-data root could not be created.
    #[error("could not create data directory {path}")]
    DataDirectory {
        /// Data root requested by server configuration.
        path: PathBuf,
        /// Underlying filesystem failure.
        #[source]
        source: std::io::Error,
    },
    /// A persisted catalog snapshot could not be loaded or validated.
    #[error("could not load project catalog from {path}")]
    Catalog {
        /// Configured data root containing the snapshot.
        path: PathBuf,
        /// Snapshot read, decode, schema, or validation failure.
        #[source]
        source: crate::project::CatalogPersistenceError,
    },
    /// The configured socket address could not be bound.
    #[error("could not bind the HTTP listener to {address}")]
    Bind {
        /// Address which was unavailable or invalid for this host.
        address: SocketAddr,
        /// Underlying network I/O failure.
        #[source]
        source: std::io::Error,
    },
    /// Axum's serving loop terminated with an I/O failure.
    #[error("the HTTP server failed")]
    Serve(#[source] std::io::Error),
}
