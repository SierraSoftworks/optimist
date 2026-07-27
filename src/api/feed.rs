//! The change feed a client watches instead of polling.
//!
//! The socket opens with the design and then carries the mutations applied to
//! it. A client applies each to the entity it names, which leaves whatever that
//! client is editing untouched, and never has to ask what changed.
//!
//! A listener that falls too far behind is told to refetch rather than being
//! fed a backlog. Holding one would spend memory to postpone an outcome instead
//! of avoiding it, and a client that far behind has to reconcile anyway.

use std::sync::Arc;

use axum::{
    Router,
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
    routing::get,
};
use serde::Serialize;
use tokio::sync::broadcast::error::RecvError;

use crate::session::{Change, Session, Snapshot, Workspace};

use super::{designs::open, error::Rejected};

pub(super) fn router() -> Router<Arc<Workspace>> {
    Router::new().route("/api/v1/designs/{design}/feed", get(subscribe))
}

/// What a watcher receives.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Update {
    /// The design as it stood when the socket opened.
    Snapshot(Snapshot),
    /// One change, to be applied to the entity it names.
    Change(Change),
    /// This listener fell behind and should fetch the design again.
    Lagged {
        /// How many changes went by unseen.
        missed: u64,
    },
}

async fn subscribe(
    State(workspace): State<Arc<Workspace>>,
    Path(design): Path<String>,
    upgrade: WebSocketUpgrade,
) -> Result<Response, Rejected> {
    let session = open(&workspace, &design)?;
    Ok(upgrade.on_upgrade(move |socket| stream(socket, session)))
}

/// Sends the design, then every change made after it.
///
/// Subscribing before taking the snapshot is what removes the gap. A change
/// landing between the two is delivered rather than lost, and a client that
/// receives one it already has can tell from the sequence and ignore it.
async fn stream(mut socket: WebSocket, session: Arc<Session>) {
    let mut changes = session.watch();
    let snapshot = session.snapshot();
    let opened = snapshot.sequence;
    if send(&mut socket, &Update::Snapshot(snapshot))
        .await
        .is_err()
    {
        return;
    }

    loop {
        let update = match changes.recv().await {
            Ok(change) if change.sequence <= opened => continue,
            Ok(change) => Update::Change(change),
            Err(RecvError::Lagged(missed)) => Update::Lagged { missed },
            Err(RecvError::Closed) => return,
        };
        if send(&mut socket, &update).await.is_err() {
            return;
        }
    }
}

async fn send(socket: &mut WebSocket, update: &Update) -> Result<(), ()> {
    let Ok(rendered) = serde_json::to_string(update) else {
        return Err(());
    };
    socket
        .send(Message::Text(rendered.into()))
        .await
        .map_err(|_| ())
}
