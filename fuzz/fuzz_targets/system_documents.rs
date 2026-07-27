#![no_main]

//! Fuzzes decoding of design documents from YAML.
//!
//! Design directories are reviewable files that people edit by hand and receive
//! through pull requests, so the deserializer sees text that no part of this
//! codebase produced. `deny_unknown_fields` makes rejection the common path; the
//! property under test is that rejection happens without panicking.

use libfuzzer_sys::fuzz_target;
use optimist::system::{ComponentDocument, SystemDocument};

/// Caps the input so the fuzzer explores document structure rather than length.
const MAX_DOCUMENT_BYTES: usize = 16 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_DOCUMENT_BYTES {
        return;
    }

    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };

    // Both document kinds share a directory and a parser, so one input is worth
    // trying against each rather than splitting the corpus across two targets.
    let _ = serde_yaml_ng::from_str::<SystemDocument>(source);
    let _ = serde_yaml_ng::from_str::<ComponentDocument>(source);
});
