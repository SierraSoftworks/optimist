//! Saying how far along a solve is while it is still running.
//!
//! # Facts, not a prediction
//!
//! A relaxation does not know how many passes it needs until it has taken them,
//! so nothing here can honestly say how long a solve will take. What the solver
//! does know at the end of every pass is where it is: which timestep of the
//! horizon it is relaxing, how many passes it has spent on it, how far the worst
//! quantity moved, and which quantity that was. Those are facts, and they are
//! what a [`Report`] carries.
//!
//! Turning them into a fraction is a presentation decision rather than a solver
//! one, and it is made once, in [`Tally`], so that a bar drawn on a terminal and
//! one drawn in a browser agree about what half way through means.
//!
//! # Reports are borrowed
//!
//! One is built at the end of every pass of every timestep of every solve in a
//! run, which for a comparison over a long horizon is hundreds of thousands. A
//! reporter that draws at ten frames a second discards nearly all of them, so a
//! report allocates nothing and costs a struct the caller can ignore.

mod tally;

use crate::system::{intervention::InterventionId, model::ComponentId};

pub use tally::{JobName, Standing, Tally};

/// Something that wants to be told how a solve is getting on.
///
/// Implementations are called from whichever thread took the pass, which is why
/// they must be [`Sync`]: a run divides its draws across threads and solves its
/// proposals alongside each other, and all of them report to the same place.
///
/// They are also expected to rate limit themselves. A pass costs on the order of
/// a millisecond, so a report is cheap against it, but drawing a terminal or
/// sending a websocket frame is not.
///
/// ```
/// use std::sync::atomic::{AtomicUsize, Ordering};
/// use optimist::system::{
///     EvaluationConfig, Solve, SystemModel, builtin_catalogue,
///     progress::{Progress, Report},
/// };
///
/// #[derive(Default)]
/// struct Passes(AtomicUsize);
///
/// impl Progress for Passes {
///     fn report(&self, _report: &Report<'_>) {
///         self.0.fetch_add(1, Ordering::Relaxed);
///     }
/// }
///
/// let model: SystemModel = serde_yaml_ng::from_str("
/// components:
///   - id: users
///     name: Users
///     type: client
///     properties:
///       request_rate: '400'
///   - id: api
///     name: API
///     type: compute
///     properties:
///       service_time: '0.02'
///       parallelism: '16'
/// relationships:
///   - from: users
///     to: api
/// ")?;
///
/// let counted = Passes::default();
/// Solve::new(&model, &builtin_catalogue()?)
///     .with(EvaluationConfig::default())
///     .reporting(&counted)
///     .evaluate()?;
///
/// assert!(counted.0.load(Ordering::Relaxed) > 0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub trait Progress: Sync {
    /// Takes one pass's worth of facts.
    fn report(&self, report: &Report<'_>);
}

/// Which solve of a run a report came from.
///
/// A comparison is several solves that finish at different moments, and a reader
/// watching one bar per proposal needs to know which is which. Borrowed for the
/// same reason a [`Report`] is; [`JobName`] is the owned form to key a map with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Job<'a> {
    /// The only solve in the run.
    Whole,
    /// The unchanged design, solved so that proposals have something to be
    /// weighed against.
    Baseline,
    /// One proposal being weighed.
    Proposed(&'a InterventionId),
}

/// What the solver knew at the end of one relaxation pass.
#[derive(Clone, Copy, Debug)]
pub struct Report<'a> {
    /// Which solve of the run this pass belongs to.
    pub job: Job<'a>,
    /// How many solves the run is made of.
    pub jobs: usize,
    /// Which share of the draws this thread is carrying, counted from zero.
    pub share: usize,
    /// How many ways the draws were divided.
    ///
    /// Shares are the same answer computed in pieces rather than pieces of the
    /// answer, so each one walks the whole horizon and the solve is finished
    /// when the slowest of them is.
    pub shares: usize,
    /// The timestep being relaxed, counted from zero.
    pub step: usize,
    /// How many timesteps the horizon holds.
    pub steps: usize,
    /// Passes taken over this timestep so far, counted from one.
    pub pass: usize,
    /// The pass count at which this timestep will give up.
    pub cap: usize,
    /// Largest relative movement of any quantity on this pass.
    ///
    /// Infinite on the first pass of a timestep, which has nothing to blend
    /// against yet.
    pub movement: f64,
    /// The movement at or below which the timestep is settled.
    pub tolerance: f64,
    /// The quantity that moved furthest, as a component and one of its channels.
    pub moving: Option<(&'a ComponentId, &'a str)>,
}

/// The parts of a report that hold still while a solve runs.
///
/// Carried down through the solve so that the pass loop, which is the only place
/// that knows how far it has got, does not also have to know which of a run's
/// solves it belongs to.
#[derive(Clone, Copy)]
pub(in crate::system) struct Reporting<'a> {
    to: Option<&'a dyn Progress>,
    job: Job<'a>,
    jobs: usize,
    share: usize,
    shares: usize,
    step: usize,
    steps: usize,
}

impl<'a> Reporting<'a> {
    /// Reports to somewhere, or nowhere.
    ///
    /// An `Option` rather than a no-op implementation so that a solve nobody is
    /// watching pays a branch the processor predicts rather than a call it
    /// cannot see through.
    pub(in crate::system) fn to(progress: Option<&'a dyn Progress>) -> Self {
        Self {
            to: progress,
            job: Job::Whole,
            jobs: 1,
            share: 0,
            shares: 1,
            step: 0,
            steps: 1,
        }
    }

    pub(in crate::system) fn on(self, job: Job<'a>, jobs: usize) -> Self {
        Self { job, jobs, ..self }
    }

    pub(in crate::system) fn sharing(self, share: usize, shares: usize) -> Self {
        Self {
            share,
            shares,
            ..self
        }
    }

    pub(in crate::system) fn at(self, step: usize, steps: usize) -> Self {
        Self {
            step,
            steps,
            ..self
        }
    }

    pub(in crate::system) fn pass(
        &self,
        pass: usize,
        cap: usize,
        movement: f64,
        tolerance: f64,
        moving: Option<(&ComponentId, &str)>,
    ) {
        let Some(to) = self.to else { return };
        to.report(&Report {
            job: self.job,
            jobs: self.jobs,
            share: self.share,
            shares: self.shares,
            step: self.step,
            steps: self.steps,
            pass,
            cap,
            movement,
            tolerance,
            moving,
        });
    }
}
