# Optimist Implementation Checklist

This file is the tracked delivery checklist. Mark an item complete only when its behavior, documentation, and relevant tests are committed. Update this file in the same commit as each completed slice.

## Quality Gate (Every Slice)

- [x] Public Rust APIs are enforced with `#![deny(missing_docs)]`.
- [x] `cargo fmt --check` passes.
- [x] `cargo test` and `cargo test --doc` pass.
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` passes.
- [x] `cargo clippy --all-targets -- -D warnings` passes for default features.
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes.
- [x] Touched production modules stay below 150 implementation lines, excluding tests and public docs.
- [x] User-facing failures include actionable `human-errors` or API advice.

## 1. Domain And Graph Foundation

- [x] Project-local short entity IDs and deterministic IndraDB UUID mapping.
- [x] Project-local case-insensitive unique names and aliases.
- [x] Typed `Outcome`, `Metric`, `Factor`, and `Intervention` vertices.
- [x] Typed edge payloads with endpoint validation and canonical edge IDs.
- [x] Embedded measurement observations with immutable correction chains.
- [x] Primitive validated distributions and dimensioned `Estimate<T>` values.
- [x] Runtime unit algebra for custom dimensions and formula validation.
- [x] Stable addresses for estimates embedded in nodes, edges, and nested Fermi components.
- [x] Typed Fermi formula AST (`Sum`, `Product`, `Ratio`, bounded transform, reference).
- [x] Quantile-based Normal/LogNormal prior elicitation with retained inputs and residual diagnostics.
- [x] Project dependence model for shared variables and residual correlations.
- [x] Typed scenario/project documents outside the causal graph.

## 2. Storage And Project Isolation

- [x] Backend-independent graph repository contract.
- [x] Deterministic in-memory repository implementation.
- [x] Generic IndraDB adapter using `MemoryDatastore`.
- [x] Atomic single-node and single-edge payload insertion/update.
- [x] Process-local project catalog with isolated repositories and counters.
- [x] Persist project catalog metadata and complete canonical project snapshots under `--data-dir`.
- [ ] Open one RocksDB database per project behind the `rocksdb` feature.
  - [ ] Resolve the local `librocksdb-sys` bindgen target mismatch (`arm64-apple-darwin` vs `aarch64-apple-darwin`) so the feature can compile on this macOS toolchain.
- [ ] Implement idempotent write-ahead `ChangeSet` recovery for multi-item mutations.
- [ ] Add forward-only schema migrations and startup integrity checks.
- [ ] Add lazy project open/close handles and idle eviction.
- [ ] Add backup/restore hooks and immutable graph snapshots.
- [ ] Run the repository contract suite against memory and temporary RocksDB.

## 3. Commands, API, And CLI

- [x] UUID-keyed idempotent commands with expected project revisions.
- [x] Project create/list/show/delete API and CLI.
- [x] Typed node create/list/get API and CLI.
- [x] Typed safe-edge create/list/get API and CLI.
- [x] Observation add/correct/list API and CLI.
- [x] Stable table, JSON, and JSONL agent output.
- [x] Revision-checked node and edge delete commands.
- [x] Typed node/edge metadata and Markdown description updates.
- [x] Primitive estimate set/show/remove commands and CLI.
- [x] Fermi component and formula authoring commands.
- [x] Scenario create/list/show/update/delete commands.
  - [x] Scenario analysis command and result transport.
- [ ] Atomic command batches and compensating undo.
- [ ] Generate OpenAPI and TypeScript contracts from Rust API types.
- [ ] Add pagination/filter/search endpoints and CLI flags.

## 4. Markdown Import And Export

- [x] Versioned `_project.md` schema foundation for project identity and base revision.
  - [ ] Extend `_project.md` with constraints, unit registry, and dependence documents.
    - [x] Persist and render project dependence documents.
    - [x] Persist, render, and cross-validate project formula documents.
- [x] Canonical entity document schema with outgoing edge payloads and Markdown description body.
- [x] Canonical `scenarios/<id>-<slug>.md` schema.
- [x] Bounded YAML frontmatter parser with path/line/column diagnostics and schema rejection.
  - [x] Deterministic in-memory rendering and parse-render-parse semantic stability.
- [x] Two-pass reference and project-constraint validation.
- [x] Safe merge import plan with create/update/unchanged/conflict reporting.
- [x] Explicit `--replace --yes` destructive restore semantics.
- [ ] Deterministic atomic directory export from one immutable revision.
  - [x] Deterministic rendered snapshots with bounded directory loading and staged rollback-aware publication.
- [x] Export-import-export semantic and byte-stability tests.
- [x] Implement `optimist project import|export` as HTTP clients.

## 5. Real-Time Collaboration

- [x] Persist committed `ChangeSet` events and replay by project revision.
  - [x] Record process-local committed `ChangeSet` events exactly once and expose ordered revision replay over API/CLI.
- [x] Per-project WebSocket subscription and ordered broadcast.
- [ ] Snapshot fallback when retained event history has a gap.
- [ ] Ephemeral anonymous presence, selection, and editing state with expiry.
- [ ] Merge disjoint nested fields and conflict on same-field edits.
- [ ] Structured base/current/proposed conflict responses.
- [ ] Two-client reconnect, retry, conflict, and stale-analysis tests.
- [ ] Pluggable authorization middleware boundary; OIDC/roles remain later hardening.

## 6. Probability And Bayesian Statistics

- [x] Exact moments and seeded sampling for Point, Normal, LogNormal, Beta, and ScaledBeta distributions.
- [x] Conjugate Beta-Binomial and Normal-Normal updates with explicit validated likelihood types.
- [x] Exact Normal sum and LogNormal product/ratio propagation including covariance and numerical variance checks.
- [x] Gaussian copula validation with positive-semidefinite correlation matrices.
- [x] Deterministic seeded joint Monte Carlo engine with pinned ChaCha20 sampling.
- [x] Formula DAG validation and evaluation with one sample per shared estimate address per draw.
- [x] Monte Carlo mean/variance standard errors, convergence criteria/status, reproducibility metadata, and invalid-sample accounting.
- [ ] Calibration history with proper scoring rules and interval coverage.
- [ ] Decomposition comparison using variance/entropy, covariance attribution, and value of information.
- [x] Law-based and analytical-vs-sampled differential tests with Monte Carlo error-derived tolerances.

## 7. Causal And Decision Analysis

- [x] Immutable analysis projection keyed by graph/scenario/dependence/formula revisions.
- [x] Finite-horizon intervention-to-outcome posterior propagation.
- [ ] Stable feedback equilibrium checks and probability of instability.
- [x] Tarjan SCC detection and bounded elementary-cycle enumeration.
- [ ] Reinforcing/balancing, nested, and interacting loop explanations.
- [x] Evidence-aware impediment ranking separate from topology-only candidates.
- [ ] Dependency-aware multidimensional intervention cost with shared prerequisite deduplication.
- [ ] Pareto impact/cost/time/risk/uncertainty frontier.
- [ ] Scalar utility only when a scenario explicitly defines conversion preferences.
- [ ] Reference model with hand-checked exact and sampled results.

## 8. Vue Workbench

- [x] Vue 3 + TypeScript + Vite scaffold with Pinia, TanStack Query, Vitest, and Playwright.
- [ ] Generated API client and project selector.
- [x] Cytoscape renderer adapter for the four core node kinds and structural edges.
- [ ] Full-viewport graph workbench with search, filters, semantic zoom, and clustering.
- [x] Typed inspector for embedded estimates, evidence, costs, and measurement histories.
  - [x] Fermi equation elicitation with quick order-of-magnitude variables, detailed PERT ranges, human unit algebra, Monte Carlo diagnostics, and explicit primitive recommendations.
  - [x] Persist and review exclusive Fermi estimate sources with server-assessed effective distributions and diagnostics.
  - [x] Metric-to-state calibration with visible observation translation and explicit estimate adoption.
- [ ] Direct graph/property editing through typed commands.
- [ ] Deterministic command bar with autocomplete, diagnostics, preview, and apply.
- [x] Explore, Impediments, Feedback, and Optimize analysis modes.
  - [x] Feedback mode with exact SCC/cycle results, bounded-result diagnostics, and graph highlighting.
  - [x] Impediments mode with separate topology and evidence-aware review ordering.
  - [x] Optimize mode with scenario creation and independent finite-horizon candidate projections.
  - [ ] Optimize mode with budget-aware candidate bundles and Pareto impact/cost frontiers.
- [x] Keyboard navigation and synchronized table/outline accessibility view.
- [x] Desktop/mobile Playwright screenshots and canvas-pixel/performance checks.
- [x] Serve production assets from Axum with SPA fallback and immutable caching.

## 9. Fuzzing, CI, And Hardening

- [x] Initialize `cargo-fuzz` with versioned seed corpora and dictionaries.
- [ ] Expand fuzzing to names and, as their feature surfaces land, YAML/Markdown, formulas/units, distributions/copulas, graph algorithms, and WebSocket events.
  - [x] Fuzz canonical `EntityId` and `EdgeId` parsing.
  - [x] Fuzz bounded JSON decoding and round trips for core tagged node, edge, and observation aggregates.
  - [x] Fuzz bounded command request decoding, deterministic in-memory sequences, and retry replay.
  - [x] Fuzz bounded YAML/Markdown frontmatter parsing.
  - [x] Fuzz bounded formula, unit, and estimate-address decoding/validation.
  - [x] Fuzz bounded distributions, formula sets, and deterministic sampling configurations.
  - [x] Fuzz bounded Gaussian dependence documents and seeded correlated draws.
- [x] Add reusable `proptest` generators for valid project/entity IDs, core node/edge/observation values, and constrained endpoints.
- [ ] Add bounded fuzz corpus regressions to pull-request CI.
- [ ] Add scheduled long fuzz, sanitizer, and expanded property-test jobs.
- [ ] Run Miri on pure safe-Rust domain/statistics code where supported.
- [ ] Add dependency audit/deny checks and parser/body/decompression limits.
- [ ] Benchmark the agreed 100-node/1,000-edge dense project fixture.
- [ ] Add tracing/metrics, automated backups, audit retention, and release packaging.
- [ ] Add TLS/reverse-proxy guidance and production OIDC viewer/editor/admin roles.
