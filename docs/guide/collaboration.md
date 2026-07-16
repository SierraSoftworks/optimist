# Collaboration and revisions

Optimist serialises project mutations through revision-checked, idempotent commands. This provides the consistency boundary used by CLI clients, agents, and the future visual workbench.

## Project and aggregate revisions

Every successful command advances the project revision. Commands carry:

- a client-generated UUID request ID,
- the project revision observed by the client,
- one typed mutation.

A repeated request ID returns the original result before revision comparison, preventing duplicate entities or observations after a transport retry.

Node, edge, scenario, formula, and dependence updates may also use their own aggregate/document revision. This catches concurrent changes to the selected object even when the caller has refreshed the wider project.

Graph mutations advance a separate graph revision. Scenario, formula, and dependence changes advance the project revision but do not pretend that graph topology changed.

## Committed ChangeSets

Each newly committed command appends one `ChangeSet` containing:

- request ID,
- base and resulting project revisions,
- resulting graph revision,
- typed command,
- committed outcome.

Replay changes strictly after an observed project revision:

```sh
cargo run -- project changes A --after 12
```

Use JSON Lines for agent ingestion:

```sh
cargo run -- --output jsonl project changes A --after 12
```

Committed event history and idempotent results are published atomically with the project snapshot under `--data-dir`. Restarting restores ordered replay and returns the original command result for a repeated pre-restart request ID.

Imported archives do not contain the source server's event log. Their retained-history floor is the archived project revision. A replay request older than that floor returns `change_history_gap` with advice to fetch a current project snapshot and reconnect from the available revision.

## WebSocket change stream

Connect to:

```text
ws://127.0.0.1:3000/api/v1/projects/A/changes/ws?after=12
```

Messages use a tagged JSON protocol:

```json
{"type":"change","value":{"request_id":"...","base_revision":12,"project_revision":13,"graph_revision":9,"command":{},"outcome":{}}}
```

After replay, the server sends:

```json
{"type":"caught_up","value":{"revision":13}}
```

Live `change` messages then continue in project-revision order.

## Race-free client startup

The server subscribes to live broadcasts before reading replay history. It then ignores queued events at or below the replay snapshot's current revision. This prevents both gaps and duplicates when a command races with connection setup.

A client should:

1. Persist its last successfully applied project revision.
2. Connect with `after=<last revision>`.
3. Apply each replayed `change` in ascending order.
4. Treat `caught_up` as the transition to live state.
5. Persist each revision only after applying the event.
6. Reconnect after transport failure using the last applied revision.

Idempotent command retries do not broadcast duplicate changes because only newly advanced project revisions are published.

## Lag recovery

Each live receiver is bounded. If it falls behind, the server sends:

```json
{"type":"replay_required","value":{"after_revision":13}}
```

The stream then closes. Reconnect from that revision to replay missing events.

Automatic snapshot fallback for a retained-history gap is not implemented yet. Persisted native projects retain every committed event; imported projects explicitly report their history floor rather than returning an incomplete replay.

## Conflict handling

Current commands reject stale project or aggregate revisions and return stable error codes with corrective advice. Field-level merge of disjoint nested changes and structured base/current/proposed conflict payloads are planned but not complete.
