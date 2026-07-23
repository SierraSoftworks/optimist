# Optimist

Optimist helps teams model complex systems, preserve uncertainty, identify feedback loops, and make investment decisions without reducing every judgement to an arbitrary confidence score.

It combines a typed causal graph, Squiggle-authored estimates, Bayesian updates, dependence-aware sampling, structural feedback analysis, an HTTP/CLI workflow, deterministic YAML projects, and ordered collaboration events.

> [!IMPORTANT]
> Optimist is under active development. The modelling and statistical core is usable today. The default server atomically snapshots complete projects under `--data-dir`, while RocksDB-backed handles, durable command replay, and several decision workflows remain in progress.

## What it provides

- **Typed systems models:** outcomes, metrics, factors, interventions, and validated structural relationships.
- **Uncertainty with units:** Squiggle programs wrapped in dimensioned estimates with deterministic seeds.
- **Composable models:** symbolic families, shared bindings, transforms, mixtures, and simulation-based calculations authored directly in Squiggle.
- **Bayesian updating:** validated Beta-Binomial and Normal-Normal conjugate updates.
- **Dependence modelling:** Gaussian copulas with positive-semidefinite correlation validation.
- **Feedback discovery:** exact strongly connected components and bounded elementary-cycle enumeration.
- **Scenario projection:** deterministic finite-horizon Monte Carlo impact for individual candidate interventions.
- **Reproducible collaboration:** revision-checked commands, idempotent retries, ordered `ChangeSet` replay, and project WebSocket streams.
- **Agent-friendly interfaces:** table, JSON, and JSONL CLI output plus typed HTTP APIs and canonical short IDs.

## Quick example

Suppose a delivery team wants to estimate how much time it spends deploying changes each month. It believes it performs between 8 and 30 deployments and that time per deployment is positive and multiplicative.

Model

$$
T_{month} = N_{deployments} \times T_{per\ deployment}
$$

directly in Squiggle:

```squiggle
deployments :: item/month = Sym.triangular(8, 18, 30)
minutes_per_deployment :: minute/item = Sym.lognormal(3.2, 0.4)
deployments * minutes_per_deployment
```

Optimist checks the resulting unit, preserves symbolic distributions when possible, and uses the estimate's seed whenever sampling is required. The project stores the Squiggle source and deterministic controls, not generated samples or summary statistics. See [examples](examples/README.md) for feedback-loop and Bayesian examples.

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
  --definition '{"source":"beta(3, 2)","seed":42,"sample_count":2048,"target_unit":{}}' \
  --provenance '["Weekly delivery review"]'
```

A fresh server allocates project `A` and entity IDs `A`, `B`, and so on. IDs are local to each project. Use `--output json` or `--output jsonl` for automation:

```sh
cargo run -- --output json --project A node list
cargo run -- --output json project changes A --after 0
cargo run -- --project A analysis structure
cargo run -- --project A scenario analyze A
```

The filesystem is the catalog under `--data-dir`. Every `projects/<ID>/` directory contains a bounded `meta.json` for cheap discovery, a complete versioned `project.yaml`, and a project-local `journal.json` only while commands await compaction. Validated commands acknowledge after the small WAL is fsynced; after a short idle period, background compaction rewrites only the touched project's snapshot and removes the covered journal prefix. `/api/v1/health` reports `pending`, `idle`, or a visible degraded persistence error. Restart replays retained requests through UUID idempotency without duplicating graph state or `ChangeSet` history. Metadata-only tombstone directories preserve monotonic project allocation after deletion. Unknown or obsolete storage schemas are rejected rather than migrated.

Create and restore immutable filesystem-catalog backups, or capture one project at an exact revision:

```sh
cargo run -- project backup create
cargo run -- project backup list
cargo run -- project backup restore <BACKUP_ID> --yes

cargo run -- project snapshot A create
cargo run -- project snapshot A list
cargo run -- --output json project snapshot A show <REVISION>
cargo run -- project snapshot A export <REVISION> ./retained-model
```

Full backups copy validated project directories and bounded backup metadata. Restore validates those directories before acquiring them as live state and automatically creates a safety backup of the projects being replaced. Project snapshots reuse the canonical project structure; creating the same revision twice is idempotent and never overwrites different content. Snapshot export publishes the selected retained revision as a deterministic YAML directory, independently of later live changes.

Apply up to 100 typed commands atomically, or submit a reviewed compensation plan for one committed batch:

```sh
cargo run -- --project A batch apply \
  --request-id 00000000-0000-4000-8000-000000000001 \
  --expected-revision 3 \
  --commands '[{"type":"delete_node","payload":{"id":"A"}}]'

cargo run -- --project A batch undo 00000000-0000-4000-8000-000000000001 \
  --request-id 00000000-0000-4000-8000-000000000002 \
  --expected-revision 4 \
  --commands '[{"type":"create_node","payload":{...}}]'
```

A failed command publishes none of the batch. Compensation never erases history: it is a second atomic batch with new project/graph revisions and `ChangeSet` events linked to the original batch. Callers provide the plan because immutable observations and externally visible actions require domain-specific correction rather than mechanical deletion.

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

- Projects, retained `ChangeSet` replay, and command idempotency results restore independently from their filesystem directories. Imported archives begin a new replay lineage at their archived revision; older cursors receive a canonical snapshot fallback through REST or WebSocket replay.
- Structural SCC/cycle analysis is exact. Finite-horizon candidate projection is implemented under documented baseline-delta assumptions, but dependence-aware dynamics, bundles, costs, stable feedback, and Pareto optimization remain pending.
- Complete canonical project archives can be exported/imported through CLI, HTTP, and the workbench. Import is full-snapshot restore; safe merge application remains pending.
- Authentication remains planned. The Vue workbench is available, but several roadmap workflows remain incomplete.

The tracked implementation status is maintained in [TODO.md](TODO.md).

## Licence

No licence has been selected in this repository yet. Treat the code as source-available for evaluation until a licence file is added.
