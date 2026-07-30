//! One design, held in memory and shared by everyone editing it.
//!
//! # Why there is no revision
//!
//! A revision exists to reject a write made against a stale read. That trade is
//! worth making when writers cannot see each other, and it is a poor one when
//! they can: every editor here receives each change as it happens, so a stale
//! read is measured in the time it takes to deliver a message rather than in
//! however long someone left a form open.
//!
//! Edits are therefore applied in arrival order and the last writer to touch a
//! given thing wins. Because a mutation names one entity rather than the whole
//! design, two people working on different components never contend at all, and
//! two people working on the same one see each other's cursor land immediately.
//! Removing optimistic concurrency removes the retry loop, the conflict dialog,
//! and the version field from every request that would have carried one.
//!
//! # Where the durability sits
//!
//! The design is authoritative in memory and written to disk as a whole
//! snapshot after a quiet period. There is no write-ahead log, because the thing
//! being protected is a document an engineer is drafting rather than a ledger:
//! losing the last few seconds of an unsaved edit is an inconvenience, and the
//! machinery that would have prevented it costs a write amplification and a
//! recovery path on every start.
//!
//! What that buys is that a snapshot on disk is always a complete, readable
//! design. There is no journal to replay, no compaction, and nothing that can be
//! half-applied, so the directory can be committed to version control and read
//! by anything that understands YAML.

mod apply;
mod mutation;
mod transfer;
mod workspace;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::RwLock,
    time::{Duration, Instant},
};

use tokio::sync::broadcast;

pub use mutation::{Mutation, MutationError};
pub use workspace::{DesignId, DesignSummary, Workspace, WorkspaceError};

use crate::system::{
    ComponentType, LoadedSystem, Mutator, SchemaError, SystemModel, read_system, write_system,
};

/// How long the design must stay untouched before it is written.
const QUIET_PERIOD: Duration = Duration::from_millis(250);

/// How many changes a slow listener may fall behind before it is disconnected.
///
/// A listener this far behind has to refetch anyway, so holding the backlog
/// would spend memory to delay an outcome rather than avoid it.
const FEED_DEPTH: usize = 256;

/// One applied change, as broadcast to everyone watching.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Change {
    /// Position in the feed.
    ///
    /// Present so a listener can tell it missed a message and refetch. It is
    /// never accepted in a request and is not a concurrency token: a client that
    /// has fallen behind resynchronises rather than having its write refused.
    pub sequence: u64,
    /// What was applied.
    pub mutation: Mutation,
}

/// The design a server is serving.
pub struct Session {
    directory: PathBuf,
    state: RwLock<State>,
    changes: broadcast::Sender<Change>,
}

struct State {
    name: String,
    summary: String,
    model: SystemModel,
    component_types: BTreeMap<String, ComponentType>,
    mutators: BTreeMap<String, Mutator>,
    sequence: u64,
    /// When the design last changed, if it has not been written since.
    touched: Option<Instant>,
    /// Whether the design behind this session has been deleted.
    discarded: bool,
}

/// A design as it stands, for answering a read.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Snapshot {
    /// Human-readable name for the design.
    pub name: String,
    /// What the design is for.
    pub summary: String,
    /// The design itself.
    pub model: SystemModel,
    /// Position in the change feed this snapshot reflects.
    pub sequence: u64,
}

impl Session {
    /// Opens the design in `directory`.
    pub fn open(directory: &Path) -> Result<Self, SchemaError> {
        let LoadedSystem {
            name,
            summary,
            model,
            component_types,
            mutators,
        } = read_system(directory)?;
        Ok(Self {
            directory: directory.to_path_buf(),
            state: RwLock::new(State {
                name,
                summary,
                model,
                component_types,
                mutators,
                sequence: 0,
                touched: None,
                discarded: false,
            }),
            changes: broadcast::channel(FEED_DEPTH).0,
        })
    }

    /// Returns the design as it stands.
    pub fn snapshot(&self) -> Snapshot {
        let state = self.read();
        Snapshot {
            name: state.name.clone(),
            summary: state.summary.clone(),
            model: state.model.clone(),
            sequence: state.sequence,
        }
    }

    /// Returns the definitions available to this design.
    pub fn catalogue(&self) -> (BTreeMap<String, ComponentType>, BTreeMap<String, Mutator>) {
        let state = self.read();
        (state.component_types.clone(), state.mutators.clone())
    }

    /// Runs `read` against the design without copying it.
    pub fn with_model<T>(
        &self,
        read: impl FnOnce(&SystemModel, &BTreeMap<String, ComponentType>) -> T,
    ) -> T {
        let state = self.read();
        read(&state.model, &state.component_types)
    }

    /// Applies one change and tells everyone watching.
    ///
    /// A mutation that would leave the design structurally broken, such as a
    /// relationship to a component that is not there, is refused. A design that
    /// is merely incomplete, such as a component still missing a property, is
    /// accepted: that is what the middle of an edit looks like.
    pub fn apply(&self, mutation: Mutation) -> Result<u64, MutationError> {
        let mut state = self.write();
        apply::apply(&mut state.model, &mutation)?;
        state.model = std::mem::take(&mut state.model).canonicalise();
        state.sequence += 1;
        state.touched = Some(Instant::now());
        let sequence = state.sequence;
        drop(state);

        // A send with no listeners is not a failure; nobody is watching yet.
        let _ = self.changes.send(Change { sequence, mutation });
        Ok(sequence)
    }

    /// Subscribes to changes made from now on.
    pub fn watch(&self) -> broadcast::Receiver<Change> {
        self.changes.subscribe()
    }

    /// Writes the design if it has been quiet long enough, reporting whether it did.
    ///
    /// Waiting for a pause rather than writing on every keystroke keeps a burst
    /// of edits to one write, and writing the whole design rather than a delta
    /// keeps what lands on disk something a person can read.
    pub fn persist_if_due(&self) -> Result<bool, SchemaError> {
        let due = {
            let state = self.read();
            !state.discarded
                && state
                    .touched
                    .is_some_and(|touched| touched.elapsed() >= QUIET_PERIOD)
        };
        if !due {
            return Ok(false);
        }
        self.persist().map(|()| true)
    }

    /// Writes the design now, whether or not it has settled.
    pub fn persist(&self) -> Result<(), SchemaError> {
        let (name, summary, model) = {
            let state = self.read();
            if state.discarded {
                return Ok(());
            }
            (
                state.name.clone(),
                state.summary.clone(),
                state.model.clone(),
            )
        };
        write_system(&self.directory, &name, &summary, &model)?;
        self.write().touched = None;
        Ok(())
    }

    /// Abandons the session, so nothing it holds is ever written again.
    ///
    /// Called when the design has been deleted. A write already in flight would
    /// otherwise recreate the directory from memory moments after it was
    /// removed, leaving a design nobody asked for and nobody can account for.
    pub fn discard(&self) {
        let mut state = self.write();
        state.discarded = true;
        state.touched = None;
    }

    /// Reports whether there are edits not yet on disk.
    pub fn pending(&self) -> bool {
        let state = self.read();
        !state.discarded && state.touched.is_some()
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, State> {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, State> {
        self.state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
