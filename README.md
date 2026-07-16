# Optimist

Optimist helps teams model complex systems, preserve uncertainty, identify feedback loops, and make investment decisions without reducing every judgement to an arbitrary confidence score.

It combines a typed causal graph, project-scoped estimates and formulas, Bayesian updates, dependence-aware Monte Carlo sampling, structural feedback analysis, an HTTP/CLI workflow, deterministic Markdown documents, and ordered collaboration events.

> [!IMPORTANT]
> Optimist is under active development. The modelling and statistical core is usable today. The default server atomically snapshots complete projects under `--data-dir`, while RocksDB-backed handles, durable command replay, and several decision workflows remain in progress.

## What it provides

- **Typed systems models:** outcomes, metrics, factors, interventions, and validated structural relationships.
- **Uncertainty with units:** Point, Normal, LogNormal, Beta, and Scaled Beta distributions wrapped in dimensioned estimates.
- **Fermi decomposition:** unit-checked formula DAGs with shared references, deterministic sampling, convergence diagnostics, and invalid-draw accounting.
- **Bayesian updating:** validated Beta-Binomial and Normal-Normal conjugate updates.
- **Dependence modelling:** Gaussian copulas with positive-semidefinite correlation validation.
- **Feedback discovery:** exact strongly connected components and bounded elementary-cycle enumeration.
- **Scenario projection:** deterministic finite-horizon Monte Carlo impact for individual candidate interventions.
- **Reproducible collaboration:** revision-checked commands, idempotent retries, ordered `ChangeSet` replay, and project WebSocket streams.
- **Agent-friendly interfaces:** table, JSON, and JSONL CLI output plus typed HTTP APIs and canonical short IDs.

## Quick example

Suppose a delivery team wants to estimate how much time it spends deploying changes each month. It believes it performs between 8 and 30 deployments and that time per deployment is positive and multiplicative.

Run the Fermi example:

```sh
cargo run --example fermi_delivery_time
```

It models

$$
T_{month} = N_{deployments} \times T_{per\ deployment}
$$

with a Scaled Beta distribution for deployment count and a LogNormal distribution for minutes per deployment. Optimist checks that the resulting unit is minutes, samples the expression with a pinned ChaCha20 stream, and reports both model variance and Monte Carlo error:

```text
Expected monthly delivery time: 693.8 minutes
Model variance: 77417.9; Monte Carlo mean SE: 1.888
Samples: 21728 valid / 21728 attempted (Converged)
```

The random seed and stopping criteria are part of the model, so the result is reproducible. See [examples](examples/README.md) for feedback-loop and Bayesian examples as well.

## Run the server and CLI

Optimist currently runs from source with Rust 1.96 or newer:

```sh
cargo build
cargo run -- server --bind 127.0.0.1:3000
```

In another terminal, create a project and its first model elements:

```sh
cargo run -- project create "Delivery reliability"

cargo run -- --project A node create \
  --kind outcome \
  --name reliable_delivery \
  --title "Reliable delivery" \
  --direction maximize

cargo run -- --project A node create \
  --kind factor \
  --name fast_feedback \
  --title "Fast feedback" \
  --controllable

cargo run -- --project A estimate set A/node/A/estimate/A \
  --slot '{"kind":"current"}' \
  --distribution '{"type":"beta","alpha":3,"beta":2}' \
  --provenance '["Weekly delivery review"]'
```

A fresh server allocates project `A` and entity IDs `A`, `B`, and so on. IDs are local to each project. Use `--output json` or `--output jsonl` for automation:

```sh
cargo run -- --output json --project A node list
cargo run -- --output json project changes A --after 0
cargo run -- --project A analysis structure
cargo run -- --project A scenario analyze A
```

The server atomically writes a versioned `catalog.json` under `--data-dir` after every successful project or command mutation. Restarting with the same data directory restores project metadata, graph contents, estimates, Fermi sources, scenarios, formulas, dependence documents, revisions, monotonic project/entity/scenario allocators, committed `ChangeSet` events, and idempotent command results. Startup rejects malformed, discontinuous, or unsupported snapshots instead of serving partial state.

### Serve the production workbench

Build the Vue application, then start Optimist from the repository root:

```sh
cd workbench
npm install
npm run build
cd ..
cargo run -- server
```

When `workbench/dist/index.html` exists, the server automatically serves the workbench and API from `http://127.0.0.1:3000`. Browser routes fall back to `index.html`; generated files under `/assets` use a one-year immutable cache, while HTML revalidates on every load. `/api` and every `/api/*` path remain JSON-only and never fall back to the SPA.

Use another build directory explicitly with either:

```sh
cargo run -- server --web-root /path/to/dist
OPTIMIST_WEB_ROOT=/path/to/dist cargo run -- server
```

Rust builds do not invoke Node. If no valid web root is configured or discovered, the server remains API-only.

## Core concepts

| Concept | Purpose |
| --- | --- |
| Outcome | A result the team wants to improve or protect. |
| Metric | A reusable measurement definition; readings live on `measures` edges. |
| Factor | A condition which influences outcomes or other factors. |
| Intervention | An investable action with uncertain cost, duration, and success. |
| Estimate | A typed distribution embedded in its owning node or edge. |
| Formula | A unit-checked Fermi decomposition of primitive estimates and components. |
| Scenario | Objectives, horizon, budgets, candidate interventions, and sampling controls. |
| Dependence model | Residual correlations between uncertain marginals using a Gaussian copula. |

Observations, costs, estimates, and evidence are embedded in their structural owner rather than represented as graph vertices. This keeps graph traversal focused on the system itself.

## Documentation

The full VuePress documentation lives in [docs](docs/README.md).

```sh
cd docs
npm install
npm run dev
```

Build the static site with `npm run build`.

Useful starting points:

- [Getting started](docs/guide/README.md)
- [Modelling systems](docs/guide/modelling.md)
- [Uncertainty and statistics](docs/guide/uncertainty.md)
- [Structural analysis](docs/guide/analysis.md)
- [Collaboration and revisions](docs/guide/collaboration.md)
- [CLI reference](docs/reference/cli.md)
- [HTTP API](docs/reference/http-api.md)

## Verify the core

```sh
cargo test
cargo test --doc
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo clippy --all-targets -- -D warnings
cargo check --examples

cd docs
npm install
npm run build
```

The nested fuzz workspace can be checked with:

```sh
cargo +nightly check --manifest-path fuzz/Cargo.toml --bins
cargo +nightly clippy --manifest-path fuzz/Cargo.toml --all-targets -- -D warnings
```

## Current limitations

- Projects, retained `ChangeSet` replay, and command idempotency results restore from one atomic canonical snapshot. Imported archives begin a new replay lineage at their archived revision; clients with older cursors receive `change_history_gap` and must fetch a current snapshot.
- The RocksDB feature is blocked on the current macOS bindgen target mismatch and is not part of the default quality gate.
- Structural SCC/cycle analysis is exact. Finite-horizon candidate projection is implemented under documented baseline-delta assumptions, but dependence-aware dynamics, bundles, costs, stable feedback, and Pareto optimization remain pending.
- Complete canonical project archives can be exported/imported through CLI, HTTP, and the workbench. Import is full-snapshot restore; safe merge application remains pending.
- Authentication and retained-history snapshot fallback remain planned. The Vue workbench is available, but several roadmap workflows remain incomplete.

The tracked implementation status is maintained in [TODO.md](TODO.md).

## Licence

No licence has been selected in this repository yet. Treat the code as source-available for evaluation until a licence file is added.
