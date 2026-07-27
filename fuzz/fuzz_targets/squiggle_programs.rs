#![no_main]

//! Fuzzes the Squiggle front end, which is the largest untrusted-text surface in
//! the system: scratchpad entries, component properties, and mutator transforms
//! all arrive as author-supplied source over the HTTP API.
//!
//! The target stops at linting rather than evaluating. Evaluation is bounded by
//! sample counts the fuzzer cannot see, so a slow program would look like a hang
//! rather than a bug. Parsing and linting are the phases that must stay total.

use libfuzzer_sys::fuzz_target;
use optimist::squiggle::{lint_program, parse};

/// Caps the input so the fuzzer spends its budget on grammar shapes rather than
/// on pathologically long but structurally trivial documents.
const MAX_SOURCE_BYTES: usize = 16 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_SOURCE_BYTES {
        return;
    }

    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };

    // A parse failure is a valid outcome; a panic or a hang is not.
    let Ok(program) = parse(source) else {
        return;
    };

    // Linting walks the whole tree and resolves identifiers, so it exercises the
    // paths where a parser that accepted something surprising would surface as an
    // index or unwrap panic further downstream.
    let _ = lint_program(&program);
});
