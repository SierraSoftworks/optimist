//! Turning a model into a plan the solver can execute repeatedly.
//!
//! Everything that does not change between iterations is resolved once here:
//! expressions are parsed, scratchpad and property values are evaluated, and
//! each component's channels are put into an order that respects the references
//! between them. A solver may then run thousands of passes without reparsing a
//! single expression or rediscovering a single dependency.
//!
//! Property values are evaluated once for a second reason. Each is drawn against
//! a seed derived from the component and property that own it, so two components
//! declaring the same service time receive independent uncertainty while any one
//! property keeps the same draws on every pass. An iteration whose inputs were
//! redrawn each time would be chasing sampling noise rather than converging.

mod channels;
mod component;
mod mutators;
mod parsing;
mod plan;
mod ports;
mod prepared;
mod properties;
mod scaling;

use crate::{
    squiggle::Value,
    system::expression::{STEP, TIME},
};

pub(super) use parsing::{clocked, runtime, syntax};
pub(super) use plan::prepare;
pub(super) use prepared::{Plan, PreparedComponent, PreparedMutator, PreparedPort};
pub(super) use properties::quantities;

/// When a plan is being resolved, and with what sampling budget.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Timing {
    pub(crate) seed: u64,
    pub(crate) ensemble: crate::squiggle::distribution::Ensemble,
    pub(crate) time: f64,
    pub(crate) step: f64,
}

impl Timing {
    /// Returns the bindings describing when this plan is being resolved.
    pub(crate) fn clock(self) -> [(&'static str, Value); 2] {
        [
            (TIME, Value::Number(self.time)),
            (STEP, Value::Number(self.step)),
        ]
    }
}
