//! Solving a design and reporting what constrains it.
//!
//! Solving is arithmetic over thousands of draws and does not belong on the
//! thread accepting requests, so it runs on the blocking pool. A model that
//! takes a moment then delays only the client that asked for it.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::{
    session::Workspace,
    system::{
        Bottleneck, Comparison, EvaluationConfig, InterventionId, bottlenecks, compare, evaluate,
        evaluate_intervention,
    },
};

use super::{designs::open, error::Rejected};

pub(super) fn router() -> Router<Arc<Workspace>> {
    Router::new()
        .route("/api/v1/designs/{design}/analysis", get(analysis))
        .route(
            "/api/v1/designs/{design}/comparisons/{intervention}",
            get(comparison),
        )
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
            ..defaults
        }
    }
}

/// A solved design and what constrains it.
#[derive(Serialize)]
struct Analysis {
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
    /// Constraints, worst first.
    bottlenecks: Vec<Bottleneck>,
}

async fn analysis(
    State(workspace): State<Arc<Workspace>>,
    Path(design): Path<String>,
    Query(controls): Query<Controls>,
) -> Result<Json<Analysis>, Rejected> {
    let session = open(&workspace, &design)?;
    let config = controls.config();
    let intervention = controls.intervention.clone();

    let analysis = tokio::task::spawn_blocking(move || {
        let snapshot = session.snapshot();
        let (types, _) = session.catalogue();
        let evaluation = match &intervention {
            Some(id) => evaluate_intervention(
                &snapshot.model,
                &types,
                &InterventionId::new(id.clone()),
                config,
            ),
            None => evaluate(&snapshot.model, &types, config),
        }?;
        let settled = evaluation.settled();
        let ranked = bottlenecks(&snapshot.model, &types, settled, config)?;
        Ok::<_, crate::system::EvaluationError>(Analysis {
            sequence: snapshot.sequence,
            converged: evaluation.converged(),
            iterations: settled.iterations,
            bottlenecks: ranked,
        })
    })
    .await
    .expect("the solver does not panic")?;
    Ok(Json(analysis))
}

async fn comparison(
    State(workspace): State<Arc<Workspace>>,
    Path((design, intervention)): Path<(String, String)>,
    Query(controls): Query<Controls>,
) -> Result<Json<Comparison>, Rejected> {
    let session = open(&workspace, &design)?;
    let config = controls.config();

    let comparison = tokio::task::spawn_blocking(move || {
        let snapshot = session.snapshot();
        let (types, _) = session.catalogue();
        compare(
            &snapshot.model,
            &types,
            &InterventionId::new(intervention),
            config,
        )
    })
    .await
    .expect("the solver does not panic")?;
    Ok(Json(comparison))
}
