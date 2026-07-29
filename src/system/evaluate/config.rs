//! How a solve is parameterised.

use crate::squiggle::distribution::Ensemble;

/// How a model should be solved.
#[derive(Clone, Copy, Debug)]
pub struct EvaluationConfig {
    /// Root of the deterministic random stream.
    pub seed: u64,
    /// Draws carried through every quantity.
    pub sample_count: usize,
    /// Number of steps to advance.
    pub horizon: usize,
    /// Length of one step, in seconds.
    pub step: f64,
    /// Cap on relaxation passes within one step.
    pub max_iterations: usize,
    /// Largest relative movement treated as settled.
    pub tolerance: f64,
    /// Fraction of the way each pass moves toward its computed value.
    pub damping: f64,
    /// Whether queues are solved for balance or advanced through time.
    pub mode: SolveMode,
    /// How many ways to divide the draws across threads.
    ///
    /// Each draw settles on its own fixed point independently of the others, so
    /// dividing them is exact rather than approximate: the pieces are the same
    /// answer, computed in parallel. One leaves the solve on the calling thread.
    pub threads: usize,
    /// Which share of the draws this solve computes.
    ///
    /// Whole unless the solver has divided the work, and callers have no reason
    /// to set it. Its size is always taken from `sample_count`, so the two cannot
    /// drift apart.
    pub share: Ensemble,
}

impl EvaluationConfig {
    /// The draws to sample, and which share of them to keep.
    ///
    /// This is what a runtime is built with, because sampling is the one place
    /// the share matters: strata are laid across the whole ensemble and only this
    /// worker's window of the result is kept.
    pub(crate) fn ensemble(self) -> Ensemble {
        self.share.resized(self.sample_count.max(1))
    }

    /// This configuration divided into one solve per thread.
    ///
    /// Dividing further than there are draws leaves some shares empty, and an
    /// empty share has nothing to sample and nothing to say, so it is dropped
    /// rather than asked to solve.
    pub(crate) fn divided(self) -> impl Iterator<Item = Self> {
        Ensemble::split(self.sample_count.max(1), self.threads.max(1))
            .filter(|share| !share.is_empty())
            .map(move |share| Self { share, ..self })
    }
}

impl Default for EvaluationConfig {
    fn default() -> Self {
        Self {
            seed: 0,
            sample_count: 1_000,
            horizon: 1,
            step: 1.0,
            // Feedback makes convergence slower than a feed-forward chain needs.
            // A retry policy against a saturated dependency has a loop gain just
            // under one, so the iterate approaches its fixed point steadily but
            // without hurry; stopping at a couple of hundred passes reports a
            // settled design as unsettled. A pass is cheap, and a loop that
            // genuinely has no fixed point diverges fast enough to be obvious
            // long before this cap.
            max_iterations: 1_500,
            tolerance: 1e-6,
            // Moving a fifth of the way rather than half. A cancelling timeout
            // and the load it relieves form an oscillator: cancelling lowers
            // utilisation, which lowers latency, which stops the cancelling,
            // which raises the load again. Half a step overshoots that on every
            // pass and the iterate cycles instead of settling. A fifth converges
            // on the same fixed point and takes more passes to get there, which
            // is the right trade when the alternative is reporting a design that
            // has a steady state as one that does not.
            damping: 0.2,
            mode: SolveMode::Steady,
            threads: 1,
            share: Ensemble::whole(0),
        }
    }
}

/// How a model's queues are solved.
///
/// The same equations either way. What differs is whether the backlog on each
/// wire is asked to balance or asked to move.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SolveMode {
    /// Solve for the backlog that balances at the current load.
    ///
    /// One algebraic solve, using the closed form for a bounded queue, so the
    /// answer arrives immediately. This is what to use while a design is being
    /// edited, and what a constraint should be read against: it says where the
    /// design comes to rest, which is the question being asked nearly all of the
    /// time.
    ///
    /// It has no memory. Where a design has more than one resting state this
    /// reports the one reachable from nothing, so a surge that would have tipped
    /// it over and left it there appears to be survived.
    #[default]
    Steady,
    /// Advance the backlog through time, one step at a time.
    ///
    /// The queue on each wire fills and drains at a finite rate, which is what
    /// gives a design memory: a buffer filled by a surge has to be emptied
    /// afterwards, and if work arrives faster than it drains the design stays
    /// where the surge left it. Hysteresis, recovery time, and whether an
    /// incident ends when its cause does are only visible here.
    ///
    /// The cost is the step. Integration is only faithful while a step is short
    /// against the time a queue takes to drain, so a horizon that reads
    /// comfortably in seconds may need thousands of steps.
    Transient,
}
