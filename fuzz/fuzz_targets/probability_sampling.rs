#![no_main]

mod json_limits;

use libfuzzer_sys::fuzz_target;
use optimist::domain::{EstimateAddress, Formula, FormulaSet, MonteCarloConfig, ProjectId};
use serde::Deserialize;

const MAX_FORMULAS: usize = 16;
const MAX_ROOTS: usize = 4;
const MAX_SAMPLES: u64 = 64;

#[derive(Deserialize)]
struct Input {
    project: ProjectId,
    #[serde(default)]
    formulas: Vec<Definition>,
    roots: Vec<Formula>,
    seed: u64,
    minimum_samples: u64,
    maximum_samples: u64,
    absolute_tolerance: f64,
    relative_tolerance: f64,
}

#[derive(Deserialize)]
struct Definition {
    address: EstimateAddress,
    formula: Formula,
}

fuzz_target!(|data: &[u8]| {
    if !json_limits::within_limits(data) {
        return;
    }
    let Ok(input) = serde_json::from_slice::<Input>(data) else {
        return;
    };
    let _ = MonteCarloConfig::new(
        input.seed,
        input.minimum_samples,
        input.maximum_samples,
        input.absolute_tolerance,
        input.relative_tolerance,
    );
    if input.formulas.len() > MAX_FORMULAS
        || input.roots.is_empty()
        || input.roots.len() > MAX_ROOTS
    {
        return;
    }
    let Ok(formulas) = FormulaSet::new(
        input
            .formulas
            .into_iter()
            .map(|definition| (definition.address, definition.formula)),
    ) else {
        return;
    };
    let minimum_samples = input.minimum_samples.clamp(2, MAX_SAMPLES);
    let maximum_samples = input.maximum_samples.clamp(minimum_samples, MAX_SAMPLES);
    let absolute_tolerance = if input.absolute_tolerance.is_finite() {
        input.absolute_tolerance.abs().max(f64::EPSILON)
    } else {
        return;
    };
    let relative_tolerance = if input.relative_tolerance.is_finite() {
        input.relative_tolerance.abs()
    } else {
        return;
    };
    let config = MonteCarloConfig::new(
        input.seed,
        minimum_samples,
        maximum_samples,
        absolute_tolerance,
        relative_tolerance,
    )
    .expect("bounded sampling config is valid");
    let first = formulas.sample_joint(&input.project, &input.roots, config);
    let second = formulas.sample_joint(&input.project, &input.roots, config);
    assert_eq!(first, second);
    if let Ok(report) = first {
        assert_eq!(
            report.diagnostics.attempted_samples,
            report.diagnostics.valid_samples + report.diagnostics.invalid_samples.total()
        );
    }
});
