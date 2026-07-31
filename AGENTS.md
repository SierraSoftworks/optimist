# Optimist Engineering Guidelines

## Running Tests

- Use `cargo nextest run` rather than `cargo test`. It runs each test in its own process and reports every failure from one run. Settings live in `.config/nextest.toml`.
- Run the narrowest set that covers the change, and only widen once it is green.
  - One binary: `cargo nextest run -E 'binary(system_catalogue)'`
  - One test: `cargo nextest run -E 'test(a_deadline_is_charged_once)'`
  - Everything touching a model: `cargo nextest run -E 'binary(/^system_/)'`
- Run `cargo nextest run` in full once before reporting the change as complete, not on every iteration.
- Doctests are not run by nextest. Run `cargo test --doc` after changing public interfaces or examples in documentation.

### Comprehensive tests

The suite is split in two, because end-to-end sweeps cost more than everything else put together.

- **Default.** `cargo nextest run` covers mechanisms: the unit tests beside the code they test, and the integration tests that read, validate, compile, and solve small models. This is what a pre-commit check runs.
- **Comprehensive.** `cargo nextest run --features comprehensive_tests` adds the binaries that solve a shipped design end to end over a long horizon — `cli`, `system_deadlines`, `system_divided`, `system_example`, `system_golden`, `system_metastable`, `system_queued_collapse`, and `system_saturation`. Each is gated with `#![cfg(feature = "comprehensive_tests")]` at the top of the file, which is where its doc comment says why.

Run the comprehensive suite when the change touched the solver, the statistics, the squiggle language, or a shipped example — and then only as a final check after the last commit and before signing the work off. Do not pass the feature on ordinary iterations.

- `tests/golden/` holds recorded solutions for the shipped examples and is comprehensive-only. When a change moves them on purpose, re-record with `UPDATE_GOLDEN=1 cargo nextest run --features comprehensive_tests -E 'binary(system_golden)'` and check the diff says what you expected it to.

### Where a test belongs

- Prefer a unit test in a `#[cfg(test)] mod tests` beside the code it covers. Parameterise the cases with `rstest` so the implementation stays short and each case reads as one claim.
- Reach for `tests/` only when the behaviour cannot be observed from inside the module: reading a design off disk, driving the binary, or a claim about a whole solved design.
- A new test that solves a shipped example belongs behind `comprehensive_tests`. If it is asserting a mechanism rather than a conclusion about a design, move it down into a unit test instead.

## Public API Documentation

- Treat public API documentation as part of the contract. Keep `#![deny(missing_docs)]` enabled at the crate root.
- Document every public module, type, trait, variant, field, constant, and function.
- Explain what the item represents, why it exists, which invariant or workflow it supports, and how callers should use it. Do not merely restate the Rust signature.
- Include runnable Rust examples on the relevant type or function boundary. Prefer one representative example that demonstrates the surrounding workflow over repetitive examples for every field.
- Describe ownership, identity scope, units, concurrency, persistence, and error/recovery semantics wherever callers could otherwise make an unsafe assumption.
- Run `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` and `cargo test --doc` after changing public interfaces.
## Implementation Comments

- Do not comment private functions, ordinary control flow, or implementation mechanics. Improve names, types, function boundaries, and data structures until the code explains itself.
- Do not use long comments to compensate for a complex implementation. Split or redesign the implementation instead.
- Keep production modules focused and strive for fewer than 150 lines of implementation, excluding tests and public API documentation.

## Statistical Mathematics

- Mathematical and statistical operations are the sole exception to the internal-comment rule.
- For every statistical algorithm, document the equations, parameterization, assumptions, support, numerical method, convergence/error criteria, limitations, and authoritative references.
- Distinguish exact analytical results from approximations and Monte Carlo estimates. Record random seeds and diagnostics required for reproducibility.
- Back mathematical claims with law-based, property, differential, and edge-case tests. Never multiply generic confidence scores or silently assume independence.
