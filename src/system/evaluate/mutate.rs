//! Rewriting a flow through the behaviours attached to a relationship.

use std::collections::BTreeMap;

use super::{config::EvaluationConfig, error::EvaluationError, flow::Direction};
use crate::{
    squiggle::{Runtime, Value},
    system::{
        compile::{Plan, PreparedMutator},
        expression::{REQUEST, RESPONSE, SIGNAL, STEP, TIME},
    },
};

/// Rewrites a flow through one attached behaviour.
///
/// Only the signals a behaviour declares are replaced; the rest travel on
/// untouched, so attaching a timeout does not silently discard the payload size
/// a downstream store needs.
///
/// Both directions are in scope. `signal` is the flow being rewritten, while
/// `demand` and `response` always name the outward and returning flows, so a
/// retry policy can raise demand in proportion to the latency coming back. Each
/// transform reads the flow as it arrived rather than as an earlier transform
/// left it, which keeps a behaviour's transforms independent of the order the
/// catalogue happens to store them in.
#[allow(clippy::too_many_arguments)]
pub(super) fn apply(
    plan: &Plan,
    mutator: &PreparedMutator,
    flow: BTreeMap<String, Value>,
    counterpart: &BTreeMap<String, Value>,
    direction: Direction,
    config: EvaluationConfig,
    time: f64,
    runtime: &mut Runtime,
) -> Result<BTreeMap<String, Value>, EvaluationError> {
    let programs = match direction {
        Direction::Request => &mutator.requests,
        Direction::Response => &mutator.responses,
    };
    if programs.is_empty() {
        return Ok(flow);
    }
    let (request, response) = match direction {
        Direction::Request => (flow.clone(), counterpart.clone()),
        Direction::Response => (counterpart.clone(), flow.clone()),
    };
    let mut scope = plan.globals.clone();
    scope.extend(mutator.properties.clone());
    scope.insert(TIME.to_owned(), Value::Number(time));
    scope.insert(STEP.to_owned(), Value::Number(config.step));
    scope.insert(SIGNAL.to_owned(), Value::dictionary(flow.clone()));
    scope.insert(REQUEST.to_owned(), Value::dictionary(request));
    scope.insert(RESPONSE.to_owned(), Value::dictionary(response));

    let mut rewritten = flow;
    for (signal, program) in programs {
        let value = runtime
            .evaluate_values(
                program,
                scope
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.clone())),
            )
            .map_err(|diagnostic| EvaluationError::Evaluation {
                location: format!("transform '{signal}' of behaviour '{}'", mutator.id),
                message: diagnostic.message,
            })?;
        rewritten.insert(signal.clone(), value);
    }
    Ok(rewritten)
}

/// What the behaviours on a wire make of the answer travelling back along it.
pub(super) struct Returning {
    /// What each behaviour sees coming back, in declaration order.
    pub(super) views: Vec<BTreeMap<String, Value>>,
    /// The answer as it reaches the caller, past every behaviour.
    pub(super) settled: BTreeMap<String, Value>,
}

/// Works the answer back along a wire, recording what each behaviour sees.
///
/// A response meets the behaviours in the reverse of the order a request does,
/// so the answer reaching the topmost one has already been rewritten by every
/// behaviour beneath it. Showing all of them the callee's raw answer instead
/// would hide a deadline from the retry policy sitting above it, and the only
/// way for that policy to learn a request had timed out would be for the callee
/// to count the cancellation as failure as well — charging one abandoned request
/// twice, and once more for every further hop its cancellation reached.
#[allow(clippy::too_many_arguments)]
pub(super) fn returning(
    plan: &Plan,
    mutators: &[PreparedMutator],
    response: BTreeMap<String, Value>,
    request: &BTreeMap<String, Value>,
    config: EvaluationConfig,
    time: f64,
    runtime: &mut Runtime,
) -> Result<Returning, EvaluationError> {
    let mut views = Vec::with_capacity(mutators.len());
    let mut seen = response;
    for mutator in mutators.iter().rev() {
        views.push(seen.clone());
        seen = apply(
            plan,
            mutator,
            seen,
            request,
            Direction::Response,
            config,
            time,
            runtime,
        )?;
    }
    views.reverse();
    Ok(Returning {
        views,
        settled: seen,
    })
}
