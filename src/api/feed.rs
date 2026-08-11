//! The change feed a client watches instead of polling.
//!
//! The socket opens with the design and then carries the mutations applied to
//! it. A client applies each to the entity it names, which leaves whatever that
//! client is editing untouched, and never has to ask what changed.
//!
//! A listener that falls too far behind is told to refetch rather than being
//! fed a backlog. Holding one would spend memory to postpone an outcome instead
//! of avoiding it, and a client that far behind has to reconcile anyway.
//!
//! # Why solves travel here too
//!
//! A solve is a fact about the design, not about whoever asked for it, and the
//! socket carrying the design's other facts is already open. Sending progress
//! along it costs no second connection and means somebody who arrives midway
//! through a long solve is told about it by the same message that tells them
//! what the design says.

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
use tokio::sync::broadcast::{Receiver, error::RecvError};

use crate::session::{Change, Session, Snapshot};

use super::{
    ApiState,
    designs::open,
    error::Rejected,
    solving::{Notice, Running, Solves, Target},
};

pub(super) fn router() -> Router<super::ApiState> {
    Router::new().route("/api/v1/designs/{design}/feed", get(subscribe))
}

/// What a watcher receives.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Update {
    /// The design as it stood when the socket opened.
    Snapshot(Snapshot),
    /// The solves already running when the socket opened.
    Active {
        /// One entry per solve, in no particular order.
        solves: Vec<Running>,
    },
    /// One change, to be applied to the entity it names.
    Change(Change),
    /// A solve started, or moved on.
    Solving {
        /// Where it has got to.
        solve: Running,
    },
    /// A solve finished, whether it answered or failed.
    Solved {
        /// Which solve it was.
        solve: Target,
    },
    /// This listener fell behind and should fetch the design again.
    Lagged {
        /// How many changes went by unseen.
        missed: u64,
    },
}

/// Everything a watcher of one design will be told, in order.
///
/// Each message is already JSON. Both transports that carry a feed send text,
/// so encoding here means an update is serialised once wherever it is bound.
pub(crate) struct Feed {
    changes: Receiver<Change>,
    notices: Receiver<Notice>,
    /// What the feed opens with, reversed so the snapshot is taken first.
    opening: Vec<Update>,
    /// Where the design stood when the snapshot was taken.
    opened: u64,
}

impl Feed {
    /// Subscribes to a design and takes the snapshot the feed opens with.
    ///
    /// Subscribing before taking the snapshot is what removes the gap. A change
    /// landing between the two is delivered rather than lost, and a client that
    /// receives one it already has can tell from the sequence and ignore it. The
    /// same holds for the solves: one that finishes between subscribing and
    /// listing is reported finished rather than left on the page.
    fn open(session: &Session, solves: &Solves) -> Self {
        let changes = session.watch();
        let notices = solves.watch();
        let snapshot = session.snapshot();
        let opened = snapshot.sequence;
        Self {
            changes,
            notices,
            opening: vec![
                Update::Active {
                    solves: solves.active(),
                },
                Update::Snapshot(snapshot),
            ],
            opened,
        }
    }

    /// The next message, or nothing once the design can send no more.
    pub(crate) async fn next(&mut self) -> Option<String> {
        let update = match self.opening.pop() {
            Some(update) => update,
            None => self.awaited().await?,
        };
        serde_json::to_string(&update).ok()
    }

    async fn awaited(&mut self) -> Option<Update> {
        loop {
            return Some(tokio::select! {
                change = self.changes.recv() => match change {
                    Ok(change) if change.sequence <= self.opened => continue,
                    Ok(change) => Update::Change(change),
                    Err(RecvError::Lagged(missed)) => Update::Lagged { missed },
                    Err(RecvError::Closed) => return None,
                },
                notice = self.notices.recv() => match notice {
                    Ok(Notice::Progress(solve)) => Update::Solving { solve },
                    Ok(Notice::Done(solve)) => Update::Solved { solve },
                    // Progress is disposable. Whatever a slow watcher missed has
                    // been superseded by the frame that comes next, and telling it
                    // to refetch the design would answer a question it did not ask.
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => return None,
                },
            });
        }
    }
}

/// Opens a feed over a design named by a client.
pub(super) fn watch(state: &ApiState, design: &str) -> Result<Feed, Rejected> {
    let session = open(&state.workspace, design)?;
    let solves = state.solving.design(design);
    Ok(Feed::open(&session, &solves))
}

async fn subscribe(
    State(state): State<ApiState>,
    Path(design): Path<String>,
    upgrade: WebSocketUpgrade,
) -> Result<Response, Rejected> {
    let feed = watch(&state, &design)?;
    Ok(upgrade.on_upgrade(move |socket| stream(socket, feed)))
}

async fn stream(mut socket: WebSocket, mut feed: Feed) {
    while let Some(message) = feed.next().await {
        if socket.send(Message::Text(message.into())).await.is_err() {
            return;
        }
    }
}
