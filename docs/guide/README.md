# Getting started

This guide builds a small delivery-reliability model using the HTTP server and CLI.

## Prerequisites

- Rust 1.96 or newer
- A local checkout of Optimist
- Two terminal windows

The default build uses an embedded in-memory IndraDB datastore and requires no external database. Under `--data-dir`, each `projects/<ID>/` directory owns cheap `meta.json` discovery metadata, complete `project.json` state, and a temporary project-local WAL. Commands return after WAL fsync; background compaction rewrites only touched projects. Unknown or obsolete schemas are rejected rather than migrated. `project backup create|list|restore` copies validated project directories, while `project snapshot <PROJECT> create|list|show|export` captures canonical project archives at exact revisions and publishes retained revisions as deterministic Markdown directories.

## Start the server

```sh
cargo run -- server --bind 127.0.0.1:3000
```

The CLI defaults to `http://127.0.0.1:3000`. Use `--server-url` or `OPTIMIST_SERVER` to select another endpoint.

To serve the production workbench from the same process, run `npm run build` inside `workbench` before starting the server. Optimist discovers `workbench/dist` from the repository root. Use `--web-root <DIRECTORY>` or `OPTIMIST_WEB_ROOT` for another Vite build; without a valid `index.html`, the server remains API-only.

## Create a project

```sh
cargo run -- project create "Delivery reliability"
```

A fresh server returns project `A`. IDs are deliberately short and scoped to the project, so a different project may also contain entity `A` without conflict.

Set the project once for the rest of the shell session:

```sh
export OPTIMIST_PROJECT=A
```

## Add an outcome and factors

```sh
cargo run -- node create \
  --kind outcome \
  --name reliable_delivery \
  --title "Reliable delivery" \
  --direction maximize

cargo run -- node create \
  --kind factor \
  --name fast_feedback \
  --title "Fast feedback" \
  --controllable

cargo run -- node create \
  --kind factor \
  --name small_batches \
  --title "Small batches" \
  --controllable
```

On a fresh project, these nodes receive IDs `A`, `B`, and `C` in creation order.

Inspect them in a script-friendly format:

```sh
cargo run -- --output json node list
```

## Add structural relationships

The current simple CLI supports safe structural relationships such as requirements, decomposition, and measurement. For example, small batches can be modelled as part of fast feedback:

```sh
cargo run -- edge create C part-of B
```

Causal `contributes`, `changes`, and `blocks` edges carry typed uncertain estimates. They are currently authored through the typed command API or the Rust library; see [modelling systems](./modelling.md) and the [HTTP API reference](../reference/http-api.md).

## Add uncertain state

First configure the outcome's native quantity, then assign estimate `A` to its `current` slot:

```sh
cargo run -- node quantity A \
  --definition '{"unit":"reliability","dimension":{"reliability":1},"aggregation":null,"support":{"type":"bounded","lower":0,"upper":1},"operational_definition":"Share of successful deliveries"}'
```

```sh
cargo run -- estimate set A/node/A/estimate/A \
  --slot '{"kind":"current"}' \
  --definition '{"source":"beta(3, 2)","seed":42,"sample_count":2048,"target_unit":{"reliability":1}}' \
  --provenance '["Weekly delivery review"]'
```

Read it back:

```sh
cargo run -- estimate show A/node/A/estimate/A
```

## Add a Fermi component

Formula components live under a primitive root address. This component references the outcome's current estimate:

```sh
cargo run -- formula set \
  A/node/A/estimate/A/component/reviewed_baseline \
  --formula '{"type":"reference","address":{"project":"A","owner":{"kind":"node","id":"A"},"estimate":"A"}}' \
  --provenance '["Baseline retained for scenario review"]'
```

Formula validation checks project scope, references, cycles, arity, bounds, and runtime units before committing the document.

## Inspect structure and history

```sh
cargo run -- analysis structure
cargo run -- project changes A --after 0
```

Structural analysis reports exact strongly connected components and bounded elementary cycles over causal edges. It does not yet estimate intervention impact or feedback stability.

Change replay returns every committed mutation in project-revision order. Repeating a command with the same request ID does not append a duplicate event.

## Next steps

- Learn the [graph and ownership model](./modelling.md).
- Add disciplined [uncertainty and Fermi estimates](./uncertainty.md).
- Understand [structural analysis boundaries](./analysis.md).
- Integrate clients with [revision replay and WebSockets](./collaboration.md).
- Run the [compileable examples](../examples/README.md).
