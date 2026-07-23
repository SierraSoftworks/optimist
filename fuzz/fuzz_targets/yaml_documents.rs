#![no_main]

use libfuzzer_sys::fuzz_target;
use optimist::project_yaml::{parse_entity, parse_project, parse_scenario};

const MAX_FUZZ_DOCUMENT_BYTES: usize = 64 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_FUZZ_DOCUMENT_BYTES {
        return;
    }
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    let _ = parse_project("fuzz/_project.yaml", input);
    let _ = parse_entity("fuzz/entities/A-entity.yaml", input);
    let _ = parse_scenario("fuzz/scenarios/A-scenario.yaml", input);
});