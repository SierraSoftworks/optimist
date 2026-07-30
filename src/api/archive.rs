//! Handing a design out as a file, and taking one back in.
//!
//! # Why the identifier is in the path on the way in
//!
//! An archive says what a design is called but not what it should be called
//! here, and letting the file decide would mean a download named after somebody
//! else's directory could land on top of a design being worked on. The client
//! names the destination, which is the same thing it does when creating a design
//! from scratch, and the server refuses to overwrite unless the request says so.

use std::sync::Arc;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;

use crate::{
    session::{DesignId, Snapshot, Workspace},
    system::MAX_ARCHIVE_BYTES,
};

use super::error::Rejected;

pub(super) fn router() -> Router<super::ApiState> {
    Router::new().route(
        "/api/v1/designs/{design}/archive",
        get(export)
            .put(import)
            // The default body limit is sized for JSON edits, and an archive is
            // the one request here that legitimately carries more.
            .layer(DefaultBodyLimit::max(MAX_ARCHIVE_BYTES as usize)),
    )
}

/// Sends the design as a zip a browser will offer to save.
async fn export(
    State(workspace): State<Arc<Workspace>>,
    Path(design): Path<String>,
) -> Result<Response, Rejected> {
    let id = DesignId::new(design)?;
    let archive = workspace.export(&id)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".to_owned()),
            // The identifier is already restricted to characters that need no
            // quoting, so there is nothing here for a filename to smuggle.
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{id}.zip\""),
            ),
        ],
        archive,
    )
        .into_response())
}

/// What an importer may say about a design it is about to overwrite.
#[derive(Debug, Default, Deserialize)]
struct ImportOptions {
    /// Replace an existing design of the same name rather than refusing.
    #[serde(default)]
    replace: bool,
}

/// Stores an uploaded archive as a design, and returns what it now holds.
///
/// A refusal to overwrite is a conflict rather than an error, because the client
/// can turn it into the one question worth asking and send the same request
/// again with an answer.
async fn import(
    State(state): State<super::ApiState>,
    Path(design): Path<String>,
    Query(options): Query<ImportOptions>,
    archive: Bytes,
) -> Result<(StatusCode, Json<Snapshot>), Rejected> {
    let id = DesignId::new(design)?;
    let replacing = options.replace;
    let session = state.workspace.import(&id, &archive, replacing)?;
    // Whatever was computed for a design of this name described the one that has
    // just been replaced.
    state.analyses.forget(id.as_str());
    state.comparisons.forget(id.as_str());
    let created = if replacing {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((created, Json(session.snapshot())))
}
