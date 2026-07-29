//! Turning a solve's reports into one figure a reader can watch.

use std::collections::BTreeMap;

use crate::system::intervention::InterventionId;

use super::{Job, Report};

/// A [`Job`] in a form that can be kept and compared.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum JobName {
    /// The only solve in the run.
    Whole,
    /// The unchanged design.
    Baseline,
    /// One proposal being weighed.
    Proposed(InterventionId),
}

impl From<Job<'_>> for JobName {
    fn from(job: Job<'_>) -> Self {
        match job {
            Job::Whole => Self::Whole,
            Job::Baseline => Self::Baseline,
            Job::Proposed(intervention) => Self::Proposed(intervention.clone()),
        }
    }
}

/// What a [`Tally`] makes of the reports it has seen.
///
/// The facts describe the least advanced part of whatever was asked about,
/// because that is what the wait is actually on: a comparison is not finished
/// when its first proposal is, and a divided solve is not finished when its
/// first share is.
#[derive(Clone, Debug, Default)]
pub struct Standing {
    /// How much of the work appears to be done, in `0..=1`.
    ///
    /// Zero also means nothing has been heard yet, which is worth distinguishing
    /// in what a reader is shown: a bar that sits at zero and one that has not
    /// started say different things.
    pub fraction: f64,
    /// The timestep it is on, counted from zero.
    pub step: usize,
    /// How many timesteps the horizon holds.
    pub steps: usize,
    /// Passes taken over that timestep.
    pub pass: usize,
    /// Largest relative movement it last reported.
    pub movement: f64,
    /// The quantity holding it up, as a component and one of its channels.
    pub moving: Option<(String, String)>,
}

/// Accumulates reports and says how far along the run they put it.
///
/// One tally serves a whole run: every solve of a comparison and every share of
/// a divided solve reports into it, and it is the tally rather than the reporter
/// that knows how those combine.
///
/// ```
/// use std::sync::Mutex;
/// use optimist::system::{
///     EvaluationConfig, Solve, SystemModel, builtin_catalogue,
///     progress::{Progress, Report, Tally},
/// };
///
/// #[derive(Default)]
/// struct Watched(Mutex<Tally>);
///
/// impl Progress for Watched {
///     fn report(&self, report: &Report<'_>) {
///         self.0.lock().expect("not poisoned").observe(report);
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
/// let watched = Watched::default();
/// Solve::new(&model, &builtin_catalogue()?)
///     .with(EvaluationConfig::default())
///     .reporting(&watched)
///     .evaluate()?;
///
/// let standing = watched.0.lock().expect("not poisoned").overall();
/// assert!(standing.fraction > 0.0);
/// assert!(standing.pass > 0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Default)]
pub struct Tally {
    jobs: BTreeMap<JobName, Solving>,
    expected: usize,
}

impl Tally {
    /// Takes one report and says where the solve it came from now stands.
    ///
    /// Cheap enough to call on every pass, and worth calling on every pass even
    /// when a reader is only shown one report in a hundred: the contraction
    /// estimate below measures against the first movement of a timestep, and
    /// sampling would measure against a later, smaller one and understate every
    /// figure that followed.
    pub fn observe(&mut self, report: &Report<'_>) -> Standing {
        self.expected = self.expected.max(report.jobs);
        let solving = self.jobs.entry(JobName::from(report.job)).or_default();
        solving.observe(report);
        solving.standing()
    }

    /// Where the run as a whole stands.
    pub fn overall(&self) -> Standing {
        let slowest = self
            .jobs
            .values()
            .map(Solving::standing)
            .min_by(|left, right| left.fraction.total_cmp(&right.fraction));
        let mut standing = slowest.unwrap_or_default();
        if self.jobs.len() < self.expected {
            standing.fraction = 0.0;
        }
        standing
    }
}

/// One solve of a run, however many shares it was divided into.
#[derive(Debug, Default)]
struct Solving {
    shares: BTreeMap<usize, Share>,
    expected: usize,
}

impl Solving {
    fn observe(&mut self, report: &Report<'_>) {
        self.expected = self.expected.max(report.shares);
        self.shares.entry(report.share).or_default().observe(report);
    }

    fn standing(&self) -> Standing {
        let slowest = self
            .shares
            .values()
            .min_by(|left, right| left.reached.total_cmp(&right.reached));
        let Some(slowest) = slowest else {
            return Standing::default();
        };
        let mut standing = slowest.latest.clone();
        // Every share walks the whole horizon, so one that has not reported yet
        // has not started, and the solve has not either.
        if self.shares.len() < self.expected {
            standing.fraction = 0.0;
        }
        standing
    }
}

/// One share of the draws, walking the horizon on its own thread.
#[derive(Debug)]
struct Share {
    reached: f64,
    step: usize,
    opened: f64,
    latest: Standing,
}

impl Default for Share {
    fn default() -> Self {
        Self {
            reached: 0.0,
            step: 0,
            opened: f64::INFINITY,
            latest: Standing::default(),
        }
    }
}

impl Share {
    fn observe(&mut self, report: &Report<'_>) {
        if report.step != self.step {
            self.step = report.step;
            self.opened = f64::INFINITY;
        }
        if !self.opened.is_finite() {
            self.opened = report.movement;
        }
        let steps = report.steps.max(1) as f64;
        let reached = (report.step as f64 + within(self.opened, report)) / steps;
        self.reached = self.reached.max(reached).clamp(0.0, 1.0);
        self.latest = Standing {
            fraction: self.reached,
            step: report.step,
            steps: report.steps,
            pass: report.pass,
            movement: report.movement,
            moving: report
                .moving
                .map(|(component, channel)| (component.to_string(), channel.to_owned())),
        };
    }
}

/// How far through one timestep a pass appears to be, in `0..=1`.
///
/// Two lower bounds are available and the larger is taken:
///
/// ```text
///   passes       p / c
///   contraction  ln(m0 / m) / ln(m0 / t)
/// ```
///
/// where `p` is the pass just taken, `c` the cap it would give up at, `m0` the
/// first finite movement of this timestep, `m` the movement just reported and
/// `t` the tolerance at or below which the timestep is settled.
///
/// The contraction estimate is the useful one. Damped relaxation converges
/// geometrically — `m_k ≈ m0·r^k` for a loop gain `r < 1` — so `ln(m0/m)` grows
/// linearly in `k` while `ln(m0/t)` is the value it must reach for the timestep
/// to settle, and their ratio is approximately the share of the passes this
/// timestep will need. The pass count is not: a design that settles in 300 of
/// 1500 passes would never show more than a fifth.
///
/// Neither is a guarantee, and this is an estimate rather than a measurement.
/// The geometric assumption holds only for a fixed rate, and `relax` changes the
/// rate whenever its adaptive damping tightens or recovers, so the estimate
/// stalls and jumps. The pass count underneath it is a true lower bound, and the
/// caller keeps the largest figure it has seen, which together give the one
/// property a reader relies on: the figure never goes backwards.
fn within(opened: f64, report: &Report<'_>) -> f64 {
    let passes = report.pass as f64 / report.cap.max(1) as f64;
    passes
        .max(contraction(opened, report.movement, report.tolerance))
        .clamp(0.0, 1.0)
}

fn contraction(opened: f64, movement: f64, tolerance: f64) -> f64 {
    let closing = opened.is_finite() && movement.is_finite() && movement < opened;
    if !closing || movement <= 0.0 || tolerance <= 0.0 || opened <= tolerance {
        // Nothing to measure against until a timestep has moved at all, and
        // nothing to reach once it opened below the tolerance.
        return f64::from(u8::from(movement <= tolerance));
    }
    (opened / movement).ln() / (opened / tolerance).ln()
}

#[cfg(test)]
mod tests {
    use crate::system::model::ComponentId;

    use super::*;

    fn report(step: usize, pass: usize, movement: f64) -> Report<'static> {
        Report {
            job: Job::Whole,
            jobs: 1,
            share: 0,
            shares: 1,
            step,
            steps: 4,
            pass,
            cap: 1_500,
            movement,
            tolerance: 1e-6,
            moving: None,
        }
    }

    #[test]
    fn a_fraction_never_goes_backwards() {
        let mut tally = Tally::default();
        let mut highest = 0.0_f64;
        // A movement that rises again is exactly what adaptive damping produces.
        for (pass, movement) in [1e-1, 1e-2, 1e-3, 5e-2, 1e-4, 1e-5].into_iter().enumerate() {
            let standing = tally.observe(&report(0, pass + 1, movement));
            assert!(standing.fraction >= highest, "fell to {}", standing.fraction);
            highest = standing.fraction;
        }
    }

    #[test]
    fn a_fraction_stays_within_its_timestep() {
        let mut tally = Tally::default();
        let standing = tally.observe(&report(1, 1_400, 1e-9));
        assert!(standing.fraction > 0.25, "{} is below step one", standing.fraction);
        assert!(standing.fraction <= 0.5, "{} is past step one", standing.fraction);
    }

    #[test]
    fn contraction_beats_counting_passes() {
        let mut tally = Tally::default();
        tally.observe(&report(0, 1, f64::INFINITY));
        tally.observe(&report(0, 2, 1.0));
        // Ten passes of a hundredth of the cap, but seven decades of the twelve
        // between the opening movement and the tolerance.
        let standing = tally.observe(&report(0, 12, 1e-7));
        assert!(standing.fraction > 0.1, "{} ignores the contraction", standing.fraction);
    }

    #[test]
    fn a_divided_solve_waits_for_its_slowest_share() {
        let mut tally = Tally::default();
        let ahead = Report {
            shares: 2,
            ..report(3, 900, 1e-9)
        };
        let behind = Report {
            share: 1,
            shares: 2,
            ..report(0, 4, 1.0)
        };
        tally.observe(&ahead);
        let standing = tally.observe(&behind);
        assert!(standing.fraction < 0.25, "{} followed the fast share", standing.fraction);
    }

    #[test]
    fn a_run_is_no_further_along_than_its_slowest_solve() {
        let intervention = InterventionId::new("bigger");
        let mut tally = Tally::default();
        tally.observe(&Report {
            job: Job::Baseline,
            jobs: 2,
            ..report(3, 900, 1e-9)
        });
        assert_eq!(tally.overall().fraction, 0.0, "one solve of two was enough");

        tally.observe(&Report {
            job: Job::Proposed(&intervention),
            jobs: 2,
            ..report(0, 4, 1.0)
        });
        assert!(tally.overall().fraction < 0.25);
    }

    #[test]
    fn the_quantity_still_moving_is_carried_through() {
        let component = ComponentId::new("api");
        let mut tally = Tally::default();
        let standing = tally.observe(&Report {
            moving: Some((&component, "utilisation")),
            ..report(0, 9, 0.5)
        });
        assert_eq!(
            standing.moving,
            Some(("api".to_owned(), "utilisation".to_owned()))
        );
    }
}
