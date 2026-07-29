//! Solving a design and reporting what constrains it.
//!
//! Solving is arithmetic over thousands of draws and does not belong on the
//! thread accepting requests, so it runs on the blocking pool. A model that
//! takes a moment then delays only the client that asked for it.
//!
//! An answer depends on the design as it stood and the controls asked for, so
//! it is remembered against both. Somebody flicking between the variants of a
//! design they are not editing is reading answers rather than recomputing them.

use std::{collections::BTreeMap, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::{
    session::Session,
    squiggle::Value,
    system::{
        Bottleneck, Comparison, EvaluationConfig, InterventionId, bottlenecks_with_mutators,
        compare_with_mutators, evaluate_intervention_with_mutators, evaluate_with_mutators,
    },
};

use super::{ApiState, designs::open, error::Rejected};

pub(super) fn router() -> Router<super::ApiState> {
    Router::new()
        .route("/api/v1/designs/{design}/analysis", get(analysis))
        .route("/api/v1/designs/{design}/preview", post(preview))
        .route(
            "/api/v1/designs/{design}/comparisons/{intervention}",
            get(comparison),
        )
}

/// An expression an author is in the middle of writing.
#[derive(Debug, Deserialize)]
struct Sketch {
    /// Squiggle source to evaluate.
    expression: String,
    /// The shared quantity being edited, if this is an edit rather than a draft.
    ///
    /// A quantity sees only the ones declared ahead of it, so naming the entry
    /// is what keeps a preview honest about what the solver will allow.
    #[serde(default)]
    entry: Option<String>,
    /// Draws to carry, bounded like every other sampling request.
    #[serde(default)]
    samples: Option<usize>,
}

/// Evaluates one expression against the design, for a preview while typing.
///
/// A `POST` because the expression is the request rather than an address: it is
/// unbounded text that changes on every keystroke, and neither a URL nor a cache
/// wants either of those properties.
async fn preview(
    State(state): State<ApiState>,
    Path(design): Path<String>,
    Json(sketch): Json<Sketch>,
) -> Result<Json<Quantity>, Rejected> {
    let session = open(&state.workspace, &design)?;
    // Fewer draws than a solve. This is a shape being glanced at while somebody
    // types, not a figure being reasoned about, and the request happens far more
    // often than a solve does.
    let sample_count = sketch.samples.unwrap_or(2_000).clamp(64, 20_000);
    let config = EvaluationConfig {
        sample_count,
        ..EvaluationConfig::default()
    };

    // Summarised inside the blocking task because an evaluated value may hold a
    // reference-counted function, which cannot cross a thread boundary. The
    // summary is plain numbers and travels freely.
    let quantity = tokio::task::spawn_blocking(move || {
        let snapshot = session.snapshot();
        let value = crate::system::preview(
            &snapshot.model,
            &sketch.expression,
            sketch.entry.as_deref(),
            config,
        )?;
        Quantity::read(&value, DRAW_BUDGET).ok_or(crate::system::EvaluationError::Evaluation {
            location: "expression".to_owned(),
            message: format!("a {} is not a quantity that can be drawn", value.type_name()),
        })
    })
    .await
    .expect("evaluation does not panic")?;
    Ok(Json(quantity))
}

/// Controls a caller may vary without editing the design.
#[derive(Debug, Deserialize)]
struct Controls {
    #[serde(default)]
    seed: Option<u64>,
    #[serde(default)]
    samples: Option<usize>,
    #[serde(default)]
    horizon: Option<usize>,
    #[serde(default)]
    step: Option<f64>,
    #[serde(default)]
    intervention: Option<String>,
    /// Whether to return every step rather than only the one it settled on.
    #[serde(default)]
    series: bool,
    /// Whether to advance queues through time rather than solve for balance.
    ///
    /// Costs a great deal more, because faithfulness needs a step short against
    /// the time a queue takes to drain, so a caller asking for this should also
    /// be asking for a shorter step and a longer horizon.
    #[serde(default)]
    transient: bool,
}

impl Controls {
    /// Bounds the sampling a caller may ask for.
    ///
    /// Draw count is the one control that costs the server rather than the
    /// caller, so it is capped. Without a ceiling a single request could occupy
    /// a worker long enough to starve everyone else editing.
    fn config(&self) -> EvaluationConfig {
        let defaults = EvaluationConfig::default();
        EvaluationConfig {
            seed: self.seed.unwrap_or(defaults.seed),
            sample_count: self
                .samples
                .unwrap_or(defaults.sample_count)
                .clamp(64, 20_000),
            horizon: self.horizon.unwrap_or(defaults.horizon).clamp(1, 500),
            step: self.step.unwrap_or(defaults.step),
            mode: if self.transient {
                crate::system::SolveMode::Transient
            } else {
                crate::system::SolveMode::Steady
            },
            ..defaults
        }
    }

    /// Renders everything about this request that changes the answer.
    ///
    /// Numbers go in as they are, floats by their bits rather than their printed
    /// form, because two step lengths that render alike must not be treated as
    /// one. Free text goes in with its length in front: an intervention is named
    /// by whoever wrote the design, and one called `a/0/0` must not be able to
    /// spell the key belonging to a different request.
    fn key(&self, sequence: u64, purpose: &str) -> String {
        let config = self.config();
        format!(
            "{sequence}/{}/{}/{}/{}/{}/{}/{}/{}",
            config.seed,
            config.sample_count,
            config.horizon,
            config.step.to_bits(),
            u8::from(self.transient),
            u8::from(self.series),
            tagged(purpose),
            tagged(self.intervention.as_deref().unwrap_or("")),
        )
    }
}

/// Writes free text so that no value of it can spell a different key.
fn tagged(text: &str) -> String {
    format!("{}:{text}", text.len())
}

/// Draws returned for each solved quantity.
///
/// A solved model may carry twenty thousand draws per channel, which is the
/// right number to compute against and far more than any chart can use. Clients
/// are sent a fixed budget instead, so the size of an answer depends on the
/// design rather than on the sampling the caller asked for.
///
/// The draws are subsampled by taking a prefix, which is uniform because a
/// sample set is shuffled when it is materialised. Taking every nth draw of the
/// sorted values would give tidier quantiles and destroy exactly the thing worth
/// looking at: where a distribution has settled on two branches, the share of
/// draws on each is the answer.
const DRAW_BUDGET: usize = 256;

/// Draws returned for each quantity within a step of a series.
///
/// Smaller than [`DRAW_BUDGET`] because a series multiplies every quantity by
/// the number of steps, and a chart of a value over time needs only enough draws
/// per point to show the shape when somebody stops on one.
const SERIES_DRAW_BUDGET: usize = 96;

/// One solved quantity, summarised and sampled.
#[derive(Serialize)]
struct Quantity {
    mean: f64,
    p10: f64,
    p50: f64,
    p90: f64,
    /// Draws, subsampled to [`DRAW_BUDGET`].
    ///
    /// Empty where the quantity is certain, which a client should read as "draw
    /// this as a point, not a spread" rather than as missing data.
    draws: Vec<f64>,
}

impl Quantity {
    fn certain(value: f64) -> Self {
        Self {
            mean: value,
            p10: value,
            p50: value,
            p90: value,
            draws: Vec::new(),
        }
    }

    fn read(value: &Value, budget: usize) -> Option<Self> {
        match value {
            Value::Number(number) => Some(Self::certain(*number)),
            Value::Distribution(distribution) => {
                let draws = distribution.samples().map_or_else(Vec::new, |samples| {
                    samples.iter().take(budget).copied().collect()
                });
                Some(Self {
                    mean: distribution.mean().ok()?,
                    p10: distribution.quantile(0.1).ok()?,
                    p50: distribution.quantile(0.5).ok()?,
                    p90: distribution.quantile(0.9).ok()?,
                    draws,
                })
            }
            _ => None,
        }
    }
}

/// Every solved quantity at one moment, by component and then by name.
///
/// Channels appear under their own names. Signals travelling on a port appear
/// under the same dotted name a manifest would use to read them, so
/// `in.requests.rate` is what arrived and `out.dependencies.latency` is what
/// came back. A client plotting a series therefore names it exactly as an author
/// writing an expression would, and the backpressure returning along a
/// relationship is as readable as the demand going out.
type Solved = BTreeMap<String, BTreeMap<String, Quantity>>;

fn solved(step: &crate::system::Step, budget: usize) -> Solved {
    step.components
        .iter()
        .map(|(id, state)| {
            let ports = state
                .arriving
                .iter()
                .map(|(port, signals)| (format!("in.{port}"), signals))
                .chain(
                    state
                        .returning
                        .iter()
                        .map(|(port, signals)| (format!("out.{port}"), signals)),
                )
                .flat_map(|(prefix, signals)| {
                    signals
                        .iter()
                        .map(move |(signal, value)| (format!("{prefix}.{signal}"), value))
                });
            let quantities = state
                .channels
                .iter()
                .map(|(name, value)| (name.clone(), value))
                .chain(ports)
                .filter_map(|(name, value)| {
                    Quantity::read(value, budget).map(|quantity| (name, quantity))
                })
                .collect();
            (id.to_string(), quantities)
        })
        .collect()
}

/// One moment in a design's history.
#[derive(Serialize)]
struct Frame {
    /// Elapsed seconds.
    time: f64,
    /// Whether relaxation settled at this step.
    converged: bool,
    /// Solved channels at this step.
    components: Solved,
}

/// A step that did not settle, and what was still moving when it stopped.
///
/// Carried instead of a bare flag because the flag alone cannot be reported
/// honestly: whether a design settled is a claim about every step of its
/// horizon, while the pass count belongs to one step, and a surge that has
/// passed leaves a design settling again in a pass or two. Pairing the two
/// produced a banner reading "did not settle after 1 passes".
#[derive(Serialize)]
struct Moving {
    /// Elapsed seconds of the step this describes.
    time: f64,
    /// Passes that step took before the solver stopped.
    iterations: usize,
    /// Component owning the quantity that was still moving furthest.
    component: String,
    /// That component's channel which was still moving furthest.
    channel: String,
    /// How far it moved on the last pass, relative to its own magnitude.
    movement: f64,
    /// Whether the iterate had stopped closing, rather than run out of passes.
    stalled: bool,
}

/// A step whose draws settled on several states rather than one.
#[derive(Serialize)]
struct Mixed {
    /// Elapsed seconds of the step this describes.
    time: f64,
    /// Component owning the quantity that settled on several values.
    component: String,
    /// That component's channel which settled on several values.
    channel: String,
    /// How many states its draws divided between.
    states: usize,
}

/// A solved design and what constrains it.
#[derive(Serialize)]
pub(super) struct Analysis {
    /// Position in the feed this answer reflects.
    ///
    /// An answer is about the design as it stood, and the design may have moved
    /// on while it was being computed. Carrying the position lets a client
    /// discard an answer that has been overtaken rather than draw a stale one.
    sequence: u64,
    /// Whether the model settled.
    converged: bool,
    /// Passes taken in the final step.
    iterations: usize,
    /// The step that settled worst, where any step failed to settle.
    #[serde(skip_serializing_if = "Option::is_none")]
    moving: Option<Moving>,
    /// Where the design settled on several states rather than one.
    #[serde(skip_serializing_if = "Option::is_none")]
    mixed: Option<Mixed>,
    /// Every solved channel, by component and then by channel.
    ///
    /// Sent alongside the ranking because a constraint's utilisation says how
    /// loaded something is and not why, and the quantities feeding it are what a
    /// reader needs to answer that without issuing a second request against a
    /// design that may have moved.
    components: Solved,
    /// Every step, oldest first, where the caller asked for them.
    ///
    /// Omitted by default. A horizon of a few hundred steps multiplies the
    /// response by the same factor, and most requests want only the state the
    /// design settled on. Each step carries fewer draws than the settled one for
    /// the same reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    series: Option<Vec<Frame>>,
    /// Constraints, worst first.
    bottlenecks: Vec<Bottleneck>,
}

async fn analysis(
    State(state): State<ApiState>,
    Path(design): Path<String>,
    Query(controls): Query<Controls>,
) -> Result<Json<Arc<Analysis>>, Rejected> {
    let session = open(&state.workspace, &design)?;
    let answers = state.analyses.design(&design);
    let key = controls.key(session.snapshot().sequence, "analysis");
    if let Some(answer) = answers.get(&key) {
        return Ok(Json(answer));
    }

    let config = controls.config();
    let intervention = controls.intervention.clone();
    let wanted = controls.series;

    let analysis =
        tokio::task::spawn_blocking(move || solve(&session, intervention, wanted, config))
            .await
            .expect("the solver does not panic")?;
    let analysis = Arc::new(analysis);
    answers.insert(key, Arc::clone(&analysis));
    Ok(Json(analysis))
}

fn solve(
    session: &Session,
    intervention: Option<String>,
    wanted: bool,
    config: EvaluationConfig,
) -> Result<Analysis, crate::system::EvaluationError> {
    let snapshot = session.snapshot();
    let (types, mutators) = session.catalogue();
    let evaluation = match &intervention {
        Some(id) => evaluate_intervention_with_mutators(
            &snapshot.model,
            &types,
            &mutators,
            &InterventionId::new(id.clone()),
            config,
        ),
        None => {
            evaluate_with_mutators(&snapshot.model, &types, &mutators, &BTreeMap::new(), config)
        }
    }?;
    let settled = evaluation.settled();
    let ranked = bottlenecks_with_mutators(&snapshot.model, &types, &mutators, settled, config)?;
    let series = wanted.then(|| {
        evaluation
            .steps
            .iter()
            .map(|step| Frame {
                time: step.time,
                converged: step.converged,
                components: solved(step, SERIES_DRAW_BUDGET),
            })
            .collect()
    });
    Ok(Analysis {
        sequence: snapshot.sequence,
        converged: evaluation.converged(),
        iterations: settled.iterations,
        moving: evaluation.unsettled().and_then(|step| {
            let moving = step.unsettled.as_ref()?;
            Some(Moving {
                time: step.time,
                iterations: step.iterations,
                component: moving.component.to_string(),
                channel: moving.channel.clone(),
                movement: moving.movement,
                stalled: moving.stalled,
            })
        }),
        mixed: evaluation.mixed().and_then(|step| {
            let mixture = step.mixture.as_ref()?;
            Some(Mixed {
                time: step.time,
                component: mixture.component.to_string(),
                channel: mixture.channel.clone(),
                states: mixture.states,
            })
        }),
        components: solved(settled, DRAW_BUDGET),
        series,
        bottlenecks: ranked,
    })
}

async fn comparison(
    State(state): State<ApiState>,
    Path((design, intervention)): Path<(String, String)>,
    Query(controls): Query<Controls>,
) -> Result<Json<Arc<Comparison>>, Rejected> {
    let session = open(&state.workspace, &design)?;
    let answers = state.comparisons.design(&design);
    let key = controls.key(
        session.snapshot().sequence,
        &format!("comparison:{intervention}"),
    );
    if let Some(answer) = answers.get(&key) {
        return Ok(Json(answer));
    }

    let config = controls.config();
    let comparison = tokio::task::spawn_blocking(move || {
        let snapshot = session.snapshot();
        let (types, mutators) = session.catalogue();
        compare_with_mutators(
            &snapshot.model,
            &types,
            &mutators,
            &InterventionId::new(intervention),
            config,
        )
    })
    .await
    .expect("the solver does not panic")?;
    let comparison = Arc::new(comparison);
    answers.insert(key, Arc::clone(&comparison));
    Ok(Json(comparison))
}
