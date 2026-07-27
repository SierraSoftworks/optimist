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

pub(super) fn router() -> Router<Arc<Workspace>> {
    Router::new()
        .route("/api/v1/designs", get(list))
        .route("/api/v1/designs/{design}", get(show))
        .route("/api/v1/designs/{design}/catalogue", get(catalogue))
        .route("/api/v1/designs/{design}/mutations", post(mutate))
}

async fn list(
    State(workspace): State<Arc<Workspace>>,
) -> Result<Json<Vec<crate::session::DesignSummary>>, Rejected> {
    Ok(Json(workspace.designs()?))
}

async fn show(
    State(workspace): State<Arc<Workspace>>,
    Path(design): Path<String>,
) -> Result<Json<Snapshot>, Rejected> {
    Ok(Json(open(&workspace, &design)?.snapshot()))
}

/// The definitions a design may draw on, shipped and project-local together.
#[derive(Serialize)]
struct Catalogue {
    component_types: BTreeMap<String, ComponentType>,
    mutators: BTreeMap<String, Mutator>,
}

async fn catalogue(
    State(workspace): State<Arc<Workspace>>,
    Path(design): Path<String>,
) -> Result<Json<Catalogue>, Rejected> {
    let (component_types, mutators) = open(&workspace, &design)?.catalogue();
    Ok(Json(Catalogue {
        component_types,
        mutators,
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
