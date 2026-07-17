# CLI reference

## Global options

```text
optimist [OPTIONS] <COMMAND>

--server-url <URL>  HTTP server, default http://127.0.0.1:3000
--project <ID>      Project used by graph/document commands
--output <FORMAT>   table, json, or jsonl
```

`OPTIMIST_SERVER` and `OPTIMIST_PROJECT` provide environment defaults.

## Command families

### Server

```sh
optimist server --bind 127.0.0.1:3000 --data-dir .optimist
optimist server --web-root workbench/dist
```

`--data-dir` stores a versioned atomic `catalog.json` snapshot containing complete canonical project archives, allocator metadata, committed `ChangeSet` events, and idempotent command results. The server restores it before binding its listener and rejects corrupt, discontinuous, or unsupported snapshots.

`--web-root` points to a completed Vite build containing `index.html`; `OPTIMIST_WEB_ROOT` provides the environment equivalent. When omitted, Optimist uses `workbench/dist` if it exists relative to the process working directory. The server gives browser routes SPA fallback, keeps `/api` JSON-only, revalidates HTML, and serves generated `/assets` with immutable caching.

### Projects

```sh
optimist project create <NAME>
optimist project list
optimist project show <PROJECT>
optimist project delete <PROJECT>
optimist project changes <PROJECT> --after <REVISION>
optimist project export <PROJECT> <DIRECTORY>
optimist project import <DIRECTORY>
optimist project import <DIRECTORY> --replace --yes
```

Export downloads one immutable canonical Markdown snapshot and publishes it through a staged directory replacement. Import validates every document and reference before restoring the archive. Restoring over an existing project requires both `--replace` and `--yes`; replacement clears process-local command/replay history.

`project changes` normally renders retained events after the requested revision. If that cursor predates retained history, the server returns a canonical snapshot fallback. Table output identifies the snapshot revision/counts, JSON includes the complete archive, and JSON Lines emits one tagged snapshot object.

### Nodes

```sh
optimist --project A node create --kind <KIND> --name <NAME> --title <TITLE> [KIND OPTIONS]
optimist --project A node get <ID>
optimist --project A node list
optimist --project A node update <ID> --title <TITLE> [--description <MARKDOWN>] [--metadata <JSON>]
optimist --project A node delete <ID>
```

Kinds and required options:

- `outcome`: `--direction maximize|minimize|target-range`
- `metric`: `--unit <UNIT>`, optional `--aggregation <TEXT>`
- `factor`: optional `--controllable`
- `intervention`: no kind-specific create options yet

Update metadata is a complete replacement, not a merge patch.

### Edges

```sh
optimist --project A edge create <SOURCE> <KIND> <DESTINATION> [KIND OPTIONS]
optimist --project A edge get <EDGE_ID>
optimist --project A edge list
optimist --project A edge update <EDGE_ID> [--description <MARKDOWN>] [--metadata <JSON>]
optimist --project A edge delete <EDGE_ID>
```

The simplified CLI creates `requires`, `part-of`, `measures`, `conflicts-with`, and `synergizes-with` edges. Typed causal payloads (`contributes`, `changes`, `blocks`) are available through commands in the Rust/HTTP API.

### Observations

```sh
optimist --project A observe add <MEASURES_EDGE> <VALUE> \
  --unit <UNIT> --observed-at <RFC3339> --source <SOURCE> \
  [--standard-deviation <SD>]

optimist --project A observe correct <MEASURES_EDGE> <OBSERVATION_ID> <VALUE>
optimist --project A observe list <MEASURES_EDGE>
```

Corrections append an immutable observation whose `supersedes` field points to the prior reading.

### Estimates

```sh
optimist --project A estimate set <ADDRESS> \
  --slot <ESTIMATE_SLOT_JSON> \
  --distribution <DISTRIBUTION_JSON> \
  [--provenance <JSON_STRING_ARRAY>]

optimist --project A estimate show <ADDRESS>
optimist --project A estimate remove <ADDRESS>
```

Canonical root address:

```text
<project>/<node|edge>/<owner>/estimate/<id>
```

Slots: `current`, `desired`, `cost`, `duration`, `probability_of_success`, `effect`, `lag`, and `degree`.

### Formulas

```sh
optimist --project A formula set <COMPONENT_ADDRESS> --formula <FORMULA_JSON>
optimist --project A formula show <COMPONENT_ADDRESS>
optimist --project A formula list
optimist --project A formula remove <COMPONENT_ADDRESS>
```

Component addresses append one or more `/component/<name>` pairs to a primitive root.

### Scenarios and dependence

```sh
optimist --project A scenario create --document <SCENARIO_DRAFT_JSON>
optimist --project A scenario show <ID>
optimist --project A scenario list
optimist --project A scenario update <ID> --revision <REV> --document <JSON>
optimist --project A scenario delete <ID> --revision <REV>
optimist --project A scenario analyze <ID>

optimist --project A dependence set --document <PROJECT_DEPENDENCE_MODEL_JSON>
optimist --project A dependence show
optimist --project A dependence remove --revision <REV>
```

`scenario analyze` evaluates each candidate independently over the scenario horizon. Table output reports objective baseline, final state, direction-oriented improvement, uncertainty, sample count, and convergence status. JSON returns the complete revision-keyed result, including improvement covariance; JSONL emits one candidate/objective row per line.

### Structural analysis

```sh
optimist --project A analysis structure \
  [--scenario <ID>] \
  [--maximum-cycle-length <N>] \
  [--maximum-cycles <N>]
```

This returns exact SCCs and bounded elementary cycles, not decision impact.

## Exit and error behaviour

The binary exits nonzero on failure and prints one `human-errors` report. HTTP errors preserve stable codes and actionable advice in CLI output.
