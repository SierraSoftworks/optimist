//! Rewriting a flow through the behaviours attached to a relationship.

use std::collections::BTreeMap;

use crate::{
    squiggle::{Runtime, Value},
    system::{
        compile::{Plan, PreparedMutator},
        expression::{REQUEST, RESPONSE, SIGNAL, STEP, TIME},
    },
};

use super::{config::EvaluationConfig, error::EvaluationError, flow::Direction};

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
    scope.insert(SIGNAL.to_owned(), Value::Dictionary(flow.clone()));
    scope.insert(REQUEST.to_owned(), Value::Dictionary(request));
    scope.insert(RESPONSE.to_owned(), Value::Dictionary(response));

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
