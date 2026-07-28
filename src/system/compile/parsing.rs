//! Parsing expressions once and drawing each quantity from its own stream.

use std::{cell::RefCell, collections::BTreeMap};

use crate::{
    squiggle::{Diagnostic, Runtime, RuntimeConfig, ast::Program, parse},
    system::{evaluate::EvaluationError, model::ComponentId},
};

/// Distinct expressions remembered before the cache is emptied and refilled.
///
/// A design's expressions number in the tens, and a process serving many designs
/// only ever holds the ones it has been asked about. The bound exists so that
/// cannot grow without limit, not because reaching it is expected.
const CACHED_PROGRAMS: usize = 4_096;

thread_local! {
    static PARSED: RefCell<BTreeMap<String, Program>> = const { RefCell::new(BTreeMap::new()) };
}

/// Parses an expression, reusing the syntax tree if it has been seen before.
///
/// A plan is resolved afresh at every step of a horizon because the values in it
/// may depend on elapsed time, but the expressions producing those values are
/// fixed by the model. Parsing them again each step dominated a long run; the
/// tree is small enough that handing back a copy costs a fraction of rebuilding
/// one.
pub(crate) fn syntax(source: &str) -> Result<Program, Vec<Diagnostic>> {
    PARSED.with_borrow_mut(|cache| {
        if let Some(program) = cache.get(source) {
            return Ok(program.clone());
        }
        let program = parse(source)?;
        if cache.len() >= CACHED_PROGRAMS {
            cache.clear();
        }
        cache.insert(source.to_owned(), program.clone());
        Ok(program)
    })
}

pub(crate) fn compile(
    component: &ComponentId,
    name: &str,
    source: &str,
) -> Result<Program, EvaluationError> {
    syntax(source).map_err(|diagnostics| EvaluationError::Syntax {
        location: format!("constraint '{name}' of component '{component}'"),
        message: first_message(&diagnostics),
    })
}

pub(crate) fn runtime(seed: u64, sample_count: usize) -> Result<Runtime, EvaluationError> {
    Runtime::with_config(RuntimeConfig {
        seed,
        sample_count,
        max_steps: 4_000_000,
    })
    .map_err(|message| EvaluationError::Evaluation {
        location: "runtime".to_owned(),
        message,
    })
}

/// Derives an independent stream for one named quantity.
///
/// Two components that declare the same service time are two separate estimates
/// and must vary independently, so the stream is keyed by owner and name rather
/// than shared. Mixing with an odd constant keeps neighbouring names from
/// producing neighbouring streams.
pub(crate) fn derive_seed(root: u64, owner: &str, name: &str) -> u64 {
    let mut hash = root ^ 0x9e37_79b9_7f4a_7c15;
    for byte in owner.bytes().chain([0]).chain(name.bytes()) {
        hash = hash.rotate_left(5) ^ u64::from(byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

pub(crate) fn first_message(diagnostics: &[Diagnostic]) -> String {
    diagnostics.first().map_or_else(
        || "invalid expression".to_owned(),
        |first| first.message.clone(),
    )
}
