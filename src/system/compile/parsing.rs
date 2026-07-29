//! Parsing expressions once and drawing each quantity from its own stream.

use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
};

use crate::{
    squiggle::{Diagnostic, Runtime, RuntimeConfig, ast::Program, names, parse},
    system::{evaluate::EvaluationError, expression::TIME, model::ComponentId},
};

/// Distinct expressions remembered before the cache is emptied and refilled.
///
/// A design's expressions number in the tens, and a process serving many designs
/// only ever holds the ones it has been asked about. The bound exists so that
/// cannot grow without limit, not because reaching it is expected.
const CACHED_PROGRAMS: usize = 4_096;

thread_local! {
    static PARSED: RefCell<BTreeMap<String, (Program, bool)>> =
        const { RefCell::new(BTreeMap::new()) };
    static CLOCKED: Cell<bool> = const { Cell::new(false) };
}

/// Parses an expression, reusing the syntax tree if it has been seen before.
///
/// A plan is resolved afresh at every step of a horizon because the values in it
/// may depend on elapsed time, but the expressions producing those values are
/// fixed by the model. Parsing them again each step dominated a long run; the
/// tree is small enough that handing back a copy costs a fraction of rebuilding
/// one.
///
/// Whether the expression reads the clock is remembered alongside the tree and
/// reported through [`clocked`], so a caller can tell whether what it just
/// compiled would differ at another point in time.
pub(crate) fn syntax(source: &str) -> Result<Program, Vec<Diagnostic>> {
    let program = PARSED.with_borrow_mut(|cache| {
        if let Some((program, clocked)) = cache.get(source) {
            return Ok((program.clone(), *clocked));
        }
        let program = parse(source)?;
        let clocked = names(source, TIME);
        if cache.len() >= CACHED_PROGRAMS {
            cache.clear();
        }
        cache.insert(source.to_owned(), (program.clone(), clocked));
        Ok((program, clocked))
    });
    program
        .inspect(|(_, clocked)| {
            if *clocked {
                CLOCKED.set(true);
            }
        })
        .map(|(program, _)| program)
}

/// Whether any expression parsed since the last call read the elapsed time.
///
/// Asking here rather than walking the model means the answer covers every
/// expression a plan is built from, including ones added to the model later:
/// they can only reach the solver by being parsed, and parsing is what is being
/// watched. Reading the answer clears it, so a caller measures the region it
/// cares about by asking once before it and once after.
pub(crate) fn clocked() -> bool {
    CLOCKED.replace(false)
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

pub(crate) fn runtime(
    seed: u64,
    ensemble: crate::squiggle::distribution::Ensemble,
) -> Result<Runtime, EvaluationError> {
    Runtime::with_config(RuntimeConfig {
        seed,
        sample_count: ensemble.size(),
        max_steps: 4_000_000,
    })
    .map(|runtime| runtime.sharing(ensemble))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_an_expression_that_reads_the_clock_is_reported() {
        clocked();
        syntax("if t > 5 then 200 else 100").expect("parses");
        assert!(clocked(), "an expression naming t reads the clock");
        assert!(!clocked(), "reading the answer clears it");
    }

    #[test]
    fn parsing_an_expression_that_ignores_the_clock_is_not_reported() {
        clocked();
        syntax("100 to 200").expect("parses");
        assert!(!clocked(), "an expression naming nothing is fixed in time");
    }

    #[test]
    fn a_remembered_expression_still_reports_reading_the_clock() {
        syntax("t * 2").expect("parses");
        clocked();
        syntax("t * 2").expect("parses");
        assert!(clocked(), "the answer comes back with the cached tree");
    }

    #[test]
    fn one_clocked_expression_among_many_is_enough() {
        clocked();
        syntax("100").expect("parses");
        syntax("t * 2").expect("parses");
        syntax("200").expect("parses");
        assert!(clocked(), "the region is clocked if any part of it is");
    }
}
