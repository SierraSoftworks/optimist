//! Listing, reading, and editing designs.

use std::{collections::BTreeMap, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::{
    session::{DesignId, Mutation, Snapshot, Workspace},
    system::{ComponentType, Mutator},
};

use super::error::Rejected;

pub(super) fn router() -> Router<super::ApiState> {
    Router::new()
        .route("/api/v1/designs", get(list).post(create))
        .route("/api/v1/designs/{design}", get(show).delete(destroy))
        .route("/api/v1/designs/{design}/catalogue", get(catalogue))
        .route("/api/v1/designs/{design}/mutations", post(mutate))
}

async fn list(
    State(workspace): State<Arc<Workspace>>,
) -> Result<Json<Vec<crate::session::DesignSummary>>, Rejected> {
    Ok(Json(workspace.designs()?))
}

/// What a client must say to start a design.
#[derive(Deserialize)]
struct NewDesign {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    summary: String,
}

/// Starts an empty design.
///
/// The identifier becomes a directory name, so it is checked against the same
/// rule that guards every other path this server builds. A design with no
/// components is a valid design: it is what somebody has after naming the thing
/// they are about to model, and refusing to store it would mean the first edit
/// had to carry the creation too.
async fn create(
    State(workspace): State<Arc<Workspace>>,
    Json(request): Json<NewDesign>,
) -> Result<(axum::http::StatusCode, Json<Snapshot>), Rejected> {
    let id = DesignId::new(request.id.clone())?;
    let name = if request.name.trim().is_empty() {
        request.id
    } else {
        request.name
    };
    let session = workspace.create(&id, &name, &request.summary)?;
    Ok((axum::http::StatusCode::CREATED, Json(session.snapshot())))
}

async fn show(
    State(workspace): State<Arc<Workspace>>,
    Path(design): Path<String>,
) -> Result<Json<Snapshot>, Rejected> {
    Ok(Json(open(&workspace, &design)?.snapshot()))
}

/// Deletes a design, along with the answers computed for it.
///
/// A design that could not be read is deletable too. Being unable to remove the
/// malformed thing cluttering the listing would leave editing a file by hand as
/// the only way out, which is the situation this server exists to avoid.
async fn destroy(
    State(state): State<super::ApiState>,
    Path(design): Path<String>,
) -> Result<axum::http::StatusCode, Rejected> {
    let id = DesignId::new(design)?;
    state.workspace.remove(&id)?;
    state.analyses.forget(id.as_str());
    state.comparisons.forget(id.as_str());
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// The definitions a design may draw on, shipped and project-local together.
#[derive(Serialize)]
struct Catalogue {
    component_types: BTreeMap<String, ComponentType>,
    mutators: BTreeMap<String, Mutator>,
    /// The quantities that travel along a relationship, by name.
    ///
    /// A port publishes signals rather than channels, so a client showing what
    /// arrived or what came back has no component type to read a unit from. This
    /// is where that unit lives.
    signals: BTreeMap<String, crate::system::Signal>,
    /// Every name an expression may call.
    ///
    /// Sent with the catalogue because an editor needs it to complete what
    /// somebody is typing, and the alternative is a copy of the language's
    /// vocabulary maintained in the client that drifts from the one the server
    /// will actually evaluate against.
    builtins: Vec<&'static str>,
}

async fn catalogue(
    State(workspace): State<Arc<Workspace>>,
    Path(design): Path<String>,
) -> Result<Json<Catalogue>, Rejected> {
    let (component_types, mutators) = open(&workspace, &design)?.catalogue();
    Ok(Json(Catalogue {
        component_types,
        mutators,
        signals: crate::system::signals(),
        builtins: crate::squiggle::builtin_names(),
    }))
}

/// A batch of changes, applied in the order they were sent.
#[derive(Deserialize)]
struct Edit {
    mutations: Vec<Mutation>,
}

/// What an editor learns about the change it made.
#[derive(Serialize)]
struct Applied {
    /// Position in the feed after the last change landed.
    ///
    /// An editor already knows what it sent, so this exists to let it recognise
    /// its own changes arriving back on the feed rather than to be sent with
    /// anything later.
    sequence: u64,
    /// How many of the submitted changes were applied.
    applied: usize,
}

/// Applies changes and tells everyone watching.
///
/// Changes apply in order and stop at the first that cannot. Earlier ones stand,
/// because each names one entity and is complete on its own; unwinding them
/// would discard work the editor meant to keep in order to tidy up a change it
/// can simply send again.
async fn mutate(
    State(workspace): State<Arc<Workspace>>,
    Path(design): Path<String>,
    Json(edit): Json<Edit>,
) -> Result<Json<Applied>, Rejected> {
    let session = open(&workspace, &design)?;
    let mut sequence = session.snapshot().sequence;
    let mut applied = 0;
    for mutation in edit.mutations {
        match session.apply(mutation) {
            Ok(next) => {
                sequence = next;
                applied += 1;
            }
            Err(error) if applied == 0 => return Err(error.into()),
            Err(_) => break,
        }
    }
    Ok(Json(Applied { sequence, applied }))
}

pub(super) fn open(
    workspace: &Workspace,
    design: &str,
) -> Result<Arc<crate::session::Session>, Rejected> {
    let id = DesignId::new(design)?;
    Ok(workspace.session(&id)?)
}
