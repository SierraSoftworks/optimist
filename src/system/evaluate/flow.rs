//! The vocabulary of a flow travelling along a relationship.

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

use crate::{
    squiggle::Value,
    system::values::{Varying, zip},
};

use super::config::EvaluationConfig;

/// Which way along a relationship a set of flows is travelling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Direction {
    /// From caller to callee: the work being asked for.
    Request,
    /// From callee to caller: how serving it went.
    Response,
}

/// Signal names the wire itself acts on.
pub(super) const RATE: &str = "rate";
pub(super) const LATENCY: &str = "latency";
pub(super) const SUCCESS: &str = "success";
pub(super) const CAPACITY: &str = "capacity";
pub(super) const PAYLOAD: &str = "payload";
/// Nodes on the far end of a relationship, supplied by the engine.
pub(super) const PEERS: &str = "peers";

/// Adds one quantity to another, draw by draw.
pub(super) fn sum(left: &Value, right: &Value, config: EvaluationConfig) -> Value {
    elementwise(left, right, config, |a, b| a + b)
}

/// Multiplies one quantity by a scale-unit boundary factor.
pub(super) fn scaled(value: &Value, factor: f64, config: EvaluationConfig) -> Value {
    if factor == 1.0 {
        return value.clone();
    }
    match value {
        Value::Number(number) => Value::Number(number * factor),
        _ => elementwise(value, &Value::Number(factor), config, |value, factor| {
            value * factor
        }),
    }
}

/// Reduces a success rate by the share that never got through.
pub(super) fn survives(success: &Value, blocked: &Value, config: EvaluationConfig) -> Value {
    elementwise(success, blocked, config, |a, b| {
        a * (1.0 - b).clamp(0.0, 1.0)
    })
}

fn elementwise(
    left: &Value,
    right: &Value,
    config: EvaluationConfig,
    combine: impl Fn(f64, f64) -> f64,
) -> Value {
    let count = config.ensemble().len();
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
    let (Some(left), Some(right)) = (
        Varying::of(left, config.ensemble(), &mut rng),
        Varying::of(right, config.ensemble(), &mut rng),
    ) else {
        return Value::Number(0.0);
    };
    zip(&left, &right, count, combine).unwrap_or(Value::Number(0.0))
}
