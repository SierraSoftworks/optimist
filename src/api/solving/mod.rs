//! Which solves are running, and how far along they are.
//!
//! # Why everyone is told
//!
//! A solve belongs to the design rather than to whoever asked for it. Two people
//! looking at the same design are waiting on the same arithmetic, and somebody
//! who reloads the page mid-solve has not stopped waiting for it. The board is
//! therefore keyed by design and broadcast to everyone watching, and a client
//! that opens the feed is told what is already running before anything new
//! happens.
//!
//! # Why a solve happens once
//!
//! Answers depend on the design as it stood and the controls asked for, and on
//! nothing else, which is what lets them be cached. The same reasoning says two
//! callers asking the same question at the same moment should wait on one solve
//! rather than start two: the second would spend a core to arrive at a value the
//! first is already computing.

mod inflight;

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use serde::Serialize;
use tokio::sync::broadcast;

use crate::system::progress::{Progress, Report, Standing, Tally};

pub(super) use inflight::InFlight;

/// How many progress frames a watcher may fall behind before they are dropped.
///
/// Small, because a frame is only worth sending while it is current. A watcher
/// that cannot keep up is better served by the next one than by the backlog.
const DEPTH: usize = 32;

/// Time between frames for one solve.
///
/// A pass takes on the order of a millisecond and a frame costs a serialisation
/// and a write per watcher, so this is what keeps a long solve from spending
/// more effort describing itself than answering.
const INTERVAL: u64 = 150;

/// Stands for a solve that has announced itself but not yet taken a pass.
const UNREPORTED: u64 = u64::MAX;

/// Which question a solve is answering.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Kind {
    /// Solving one variant and ranking what it runs out of.
    Analysis,
    /// Weighing one variant against the design as it stands.
    Comparison,
}

/// What is being solved, in the terms a reader sees on the page.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct Target {
    /// Which question is being answered.
    pub(super) kind: Kind,
    /// The variant being solved, or none for the design as it stands.
    pub(super) variant: Option<String>,
    /// Where the design stood when this solve started.
    ///
    /// Carried so that a client can tell an answer about the design in front of
    /// it from one about the design as it was two edits ago. It is not something
    /// to match on: a solve that started before the last keystroke is still
    /// running, and saying so is still true.
    pub(super) sequence: u64,
}

/// One running solve, as a watcher sees it.
#[derive(Clone, Debug, Serialize)]
pub(super) struct Running {
    #[serde(flatten)]
    pub(super) target: Target,
    /// How much of the solve appears to be done, in `0..=1`.
    pub(super) fraction: f64,
    /// The timestep it is relaxing, counted from one.
    pub(super) step: usize,
    /// How many timesteps the horizon holds.
    pub(super) steps: usize,
    /// Passes taken over that timestep.
    pub(super) pass: usize,
    /// The quantity holding it up.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) moving: Option<Moving>,
}

/// The quantity a relaxation is still waiting on.
#[derive(Clone, Debug, Serialize)]
pub(super) struct Moving {
    /// The component it belongs to.
    pub(super) component: String,
    /// Which of that component's channels it is.
    pub(super) channel: String,
}

/// What watchers are told as solves come and go.
#[derive(Clone, Debug)]
pub(super) enum Notice {
    /// A solve started, or moved on.
    Progress(Running),
    /// A solve finished, whether it answered or failed.
    Done(Target),
}

/// The solves running for every design a server has been asked about.
#[derive(Default)]
pub(super) struct Board {
    designs: Mutex<HashMap<String, Arc<Solves>>>,
}

impl Board {
    /// Returns the solves running for one design, creating the entry if needed.
    pub(super) fn design(&self, design: &str) -> Arc<Solves> {
        let mut designs = self.designs.lock().unwrap_or_else(|held| held.into_inner());
        Arc::clone(designs.entry(design.to_owned()).or_default())
    }
}

/// The solves running for one design.
pub(super) struct Solves {
    active: Mutex<HashMap<String, Running>>,
    notices: broadcast::Sender<Notice>,
}

impl Default for Solves {
    fn default() -> Self {
        Self {
            active: Mutex::new(HashMap::new()),
            notices: broadcast::channel(DEPTH).0,
        }
    }
}

impl Solves {
    /// Everything running for this design right now.
    pub(super) fn active(&self) -> Vec<Running> {
        self.active
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .values()
            .cloned()
            .collect()
    }

    /// Subscribes to solves starting, moving and finishing.
    pub(super) fn watch(&self) -> broadcast::Receiver<Notice> {
        self.notices.subscribe()
    }

    fn set(&self, key: &str, running: Running) {
        self.active
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .insert(key.to_owned(), running.clone());
        let _ = self.notices.send(Notice::Progress(running));
    }

    fn clear(&self, key: &str, target: Target) {
        self.active
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .remove(key);
        let _ = self.notices.send(Notice::Done(target));
    }
}

/// Publishes one solve's progress to everyone watching its design.
///
/// Held by the blocking task that does the solving, so the solve is announced
/// while it runs and taken off the board when the task ends — including when it
/// ends by panicking, which is the case a watcher would otherwise be left
/// waiting on forever.
pub(super) struct Reporter {
    solves: Arc<Solves>,
    key: String,
    target: Target,
    steps: usize,
    started: Instant,
    /// Milliseconds since the solve started at which a frame last went out.
    sent: AtomicU64,
    tally: Mutex<Tally>,
}

impl Reporter {
    pub(super) fn new(solves: Arc<Solves>, key: String, target: Target, steps: usize) -> Self {
        let reporter = Self {
            solves,
            key,
            target,
            steps: steps.max(1),
            started: Instant::now(),
            sent: AtomicU64::new(UNREPORTED),
            tally: Mutex::new(Tally::default()),
        };
        // Announced before the first pass, because what a watcher most wants to
        // know is that something has started. The horizon comes from the
        // configuration rather than from a report so that this first frame says
        // how much there is to do rather than guessing at one step.
        reporter.publish(&Standing::default());
        reporter
    }

    fn publish(&self, standing: &Standing) {
        self.solves.set(
            &self.key,
            Running {
                target: self.target.clone(),
                fraction: standing.fraction,
                step: standing.step + 1,
                steps: self.steps,
                pass: standing.pass,
                moving: standing.moving.as_ref().map(|(component, channel)| Moving {
                    component: component.clone(),
                    channel: channel.clone(),
                }),
            },
        );
    }
}

impl Progress for Reporter {
    fn report(&self, report: &Report<'_>) {
        let standing = self
            .tally
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .observe(report);
        let now = self.started.elapsed().as_millis() as u64;
        let sent = self.sent.load(Ordering::Relaxed);
        // The first pass always goes out. Until one has, the only thing anybody
        // has been told is that a solve exists.
        if sent != UNREPORTED && now < sent + INTERVAL {
            return;
        }
        self.sent.store(now, Ordering::Relaxed);
        self.publish(&standing);
    }
}

impl Drop for Reporter {
    fn drop(&mut self) {
        self.solves.clear(&self.key, self.target.clone());
    }
}
