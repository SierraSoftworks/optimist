//! Drawing a solve's progress while it runs.
//!
//! # Why standard error
//!
//! A report goes to standard output so that `optimist bottlenecks | jq` works,
//! and a bar drawn into that stream would be part of the answer. It is drawn to
//! standard error instead, which is also where it belongs: it is not output, it
//! is the tool saying it has not finished.
//!
//! # Why it does not appear immediately
//!
//! Most solves are over before a reader could read anything, and a bar that
//! flashes up and vanishes is worse than no bar. Nothing is drawn until a solve
//! has gone on long enough to be worth explaining.

mod stderr;

use std::{
    collections::BTreeMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use clap::ValueEnum;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

use crate::system::progress::{JobName, Progress, Report, Standing, Tally};

/// How long a solve must have been running before anything is drawn.
const PATIENCE: Duration = Duration::from_millis(250);

/// Time between redraws of one bar.
///
/// A pass takes on the order of a millisecond, so without this the tool would
/// spend more time formatting captions than solving.
const INTERVAL: u64 = 60;

/// How a bar is laid out, once for every solve of a run.
const LAYOUT: &str = "{prefix:>16} [{bar:24}] {percent:>3}% {msg}";

/// When to draw a progress bar.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub(super) enum ProgressChoice {
    /// Draw on a terminal, and nowhere else.
    #[default]
    Auto,
    /// Draw wherever standard error goes, terminal or not.
    Always,
    /// Never draw.
    Never,
}

/// One bar per solve of a run, drawn from whichever thread is solving.
pub(super) struct Bars {
    bars: MultiProgress,
    hidden: bool,
    started: Instant,
    drawing: Mutex<Drawing>,
}

struct Drawing {
    tally: Tally,
    drawn: BTreeMap<JobName, Drawn>,
}

struct Drawn {
    bar: ProgressBar,
    /// Milliseconds since the run started at which this bar was last redrawn.
    last: u64,
}

impl Bars {
    pub(super) fn new(choice: ProgressChoice) -> Self {
        let bars = MultiProgress::with_draw_target(match choice {
            ProgressChoice::Auto => ProgressDrawTarget::stderr(),
            ProgressChoice::Always => {
                ProgressDrawTarget::term_like(Box::new(stderr::Stderr))
            }
            ProgressChoice::Never => ProgressDrawTarget::hidden(),
        });
        let hidden = bars.is_hidden();
        Self {
            bars,
            hidden,
            started: Instant::now(),
            drawing: Mutex::new(Drawing {
                tally: Tally::default(),
                drawn: BTreeMap::new(),
            }),
        }
    }
}

impl Progress for Bars {
    fn report(&self, report: &Report<'_>) {
        if self.hidden {
            return;
        }
        let elapsed = self.started.elapsed();
        let mut drawing = self.drawing.lock().expect("no reporter panics while holding this");
        let standing = drawing.tally.observe(report);
        if elapsed < PATIENCE {
            return;
        }

        let now = elapsed.as_millis() as u64;
        let name = JobName::from(report.job);
        if let Some(drawn) = drawing.drawn.get_mut(&name) {
            if now < drawn.last + INTERVAL {
                return;
            }
            drawn.last = now;
            drawn.bar.set_position(position(&standing));
            drawn.bar.set_message(caption(&standing));
            return;
        }

        let bar = self.bars.add(ProgressBar::new(1_000));
        bar.set_style(
            ProgressStyle::with_template(LAYOUT)
                .expect("the layout is a constant")
                .progress_chars("=> "),
        );
        bar.set_prefix(label(&name, report.jobs));
        bar.set_position(position(&standing));
        bar.set_message(caption(&standing));
        drawing.drawn.insert(name, Drawn { bar, last: now });
    }
}

/// Takes the bars down again, whether the run answered or failed.
impl Drop for Bars {
    fn drop(&mut self) {
        let drawing = self
            .drawing
            .get_mut()
            .expect("no reporter panics while holding this");
        for drawn in drawing.drawn.values() {
            drawn.bar.finish_and_clear();
        }
        let _ = self.bars.clear();
    }
}

fn position(standing: &Standing) -> u64 {
    (standing.fraction.clamp(0.0, 1.0) * 1_000.0) as u64
}

/// What to call one solve of a run.
///
/// A run of one needs no label beyond what it is doing; a comparison needs each
/// bar to say which proposal it is weighing.
fn label(name: &JobName, jobs: usize) -> String {
    match name {
        JobName::Whole => "solving".to_owned(),
        _ if jobs <= 1 => "solving".to_owned(),
        JobName::Baseline => "as designed".to_owned(),
        JobName::Proposed(intervention) => intervention.to_string(),
    }
}

fn caption(standing: &Standing) -> String {
    let mut caption = if standing.steps > 1 {
        format!(
            "step {}/{} · pass {}",
            standing.step + 1,
            standing.steps,
            standing.pass
        )
    } else {
        format!("pass {}", standing.pass)
    };
    if let Some((component, channel)) = &standing.moving {
        caption.push_str(&format!(" · waiting on {component}.{channel}"));
    }
    caption
}
