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

`--data-dir` stores a versioned atomic `catalog.json` snapshot containing complete canonical project archives, allocator metadata, committed `ChangeSet` events, and idempotent command results. Validated commands are written ahead to `command-journal.json`; startup idempotently completes any retained request before binding its listener. Schema v1 snapshots migrate to v2 by making retained replay floors and event lists explicit, then are atomically rewritten after full validation. Corrupt, discontinuous, unknown older, and future catalog or journal schemas are rejected without modifying the original files.

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
optimist project backup create
optimist project backup list
optimist project backup restore <BACKUP_ID> --yes
optimist project snapshot <PROJECT> create
optimist project snapshot <PROJECT> list
optimist project snapshot <PROJECT> show <REVISION>
```

Export downloads one immutable canonical Markdown snapshot and publishes it through a staged directory replacement. Import validates every document and reference before restoring the archive. Restoring over an existing project requires both `--replace` and `--yes`; replacement clears process-local command/replay history.

`project changes` normally renders retained events after the requested revision. If that cursor predates retained history, the server returns a canonical snapshot fallback. Table output identifies the snapshot revision/counts, JSON includes the complete archive, and JSON Lines emits one tagged snapshot object.

Catalog backups retain complete project archives, replay history, retry results, and allocator state. Restore validates the selected backup before changing live state, requires `--yes`, and creates an immutable safety backup of the catalog being replaced. Project snapshots retain one canonical archive at an exact revision; repeated creation is idempotent, and `show` returns that archive without changing the live project.

### Command batches

```sh
optimist --project A batch apply \
  --request-id <UUID> \
  --expected-revision <REVISION> \
  --commands '<GRAPH_COMMAND_JSON_ARRAY>'

optimist --project A batch undo <ORIGINAL_BATCH_UUID> \
  --request-id <NEW_UUID> \
  --expected-revision <CURRENT_REVISION> \
  --commands '<COMPENSATION_COMMAND_JSON_ARRAY>'
```

Batch request IDs derive stable child command IDs, so an exact retry returns the original result. A batch contains 1 to 100 commands and commits all commands in order or none. Reusing an ID with different content is rejected.

Undo requires a caller-reviewed compensation plan. The plan commits as a new atomic batch and advances normal project, graph, and aggregate revisions; it does not delete immutable facts or rewrite replay history. Only retained forward batches may be compensated, and each forward batch accepts one compensation batch.

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
  [--provenance <JSON_STRING_ARRAY>] \
  [--uncertainty <ESTIMATE_UNCERTAINTY_JSON>]

optimist --project A estimate show <ADDRESS>
optimist --project A estimate remove <ADDRESS>
```

Canonical root address:

```text
<project>/<node|edge>/<owner>/estimate/<id>
```

Slots: `current`, `desired`, `cost`, `duration`, `probability_of_success`, `effect`, `lag`, and `degree`.

Uncertainty JSON accepts optional `epistemic`, `process`, and `measurement` strings. They retain distinct assumptions and do not alter or decompose the effective distribution.

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
