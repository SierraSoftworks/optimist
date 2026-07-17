# Getting started

This guide builds a small delivery-reliability model using the HTTP server and CLI.

## Prerequisites

- Rust 1.96 or newer
- A local checkout of Optimist
- Two terminal windows

The default build uses an embedded in-memory IndraDB datastore and requires no external database. The server atomically snapshots complete canonical projects, ordered committed changes, idempotent command results, and allocator state under `--data-dir` after successful mutations and restores them on restart. Known older catalog schemas migrate forward during startup only after full integrity validation; unsupported or invalid snapshots stop startup and remain untouched. `project backup create|list|restore` manages immutable full-catalog backups, while `project snapshot <PROJECT> create|list|show` captures canonical project archives at exact revisions.

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

An `EstimateAddress` includes the project, owner, and owner-local estimate ID. The following command assigns estimate `A` to the `current` slot on outcome node `A`:

```sh
cargo run -- estimate set A/node/A/estimate/A \
  --slot '{"kind":"current"}' \
  --distribution '{"type":"beta","alpha":3,"beta":2}' \
  --provenance '["Weekly delivery review"]'
```

The Beta distribution is supported on $[0,1]$, which matches a normalized state. Optimist rejects an unbounded Normal distribution in this slot rather than silently clipping it.

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
