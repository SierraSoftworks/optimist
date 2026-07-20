# HTTP and WebSocket API

The server defaults to `http://127.0.0.1:3000`. JSON mutation requests use the revision-checked command endpoint; read routes expose complete typed aggregates.

## Health

```http
GET /api/v1/health
```

## Projects

```http
POST   /api/v1/projects
GET    /api/v1/projects
GET    /api/v1/projects/{project}
DELETE /api/v1/projects/{project}
GET    /api/v1/projects/{project}/changes?after={revision}
GET    /api/v1/projects/{project}/archive
POST   /api/v1/project-archives?replace={bool}&yes={bool}
```

Create body:

```json
{"name":"Delivery reliability"}
```

Archive responses are bounded JSON envelopes containing validated project metadata, summary counts, and a `files` map of canonical Markdown paths to UTF-8 content. Import validates envelope metadata, file/path/byte limits, every Markdown document, cross-references, formulas, and dependence before atomically publishing a fresh in-memory project entry. Existing IDs return `project_import_requires_replace` unless both replacement flags are true.

## Backups and immutable snapshots

```http
POST /api/v1/backups
GET  /api/v1/backups
POST /api/v1/backups/{backup}/restore?yes=true

POST /api/v1/projects/{project}/snapshots
GET  /api/v1/projects/{project}/snapshots
GET  /api/v1/projects/{project}/snapshots/{revision}
```

Backup routes are available only on a server configured with persistent `--data-dir` storage. A full backup is an immutable copy of the validated versioned catalog plus bounded metadata. Restore first validates the selected catalog, then creates a safety backup of current state, atomically publishes the replacement, and closes existing project change streams so clients reconnect against restored revisions. Omitting `yes=true` returns `backup_restore_requires_confirmation` without changing state.

Project snapshots are canonical `ProjectArchive` documents keyed by project and revision. Creating a snapshot at an already captured revision is idempotent when the content matches and never overwrites different content.

## Commands

```http
POST /api/v1/projects/{project}/commands
```

Envelope:

```json
{
  "request_id": "00000000-0000-0000-0000-000000000000",
  "expected_revision": 12,
  "command": {
    "type": "delete_node",
    "payload": {"id": "A"}
  }
}
```

A retry must reuse the same `request_id`. The original result is returned without appending another ChangeSet.

Successful response:

```json
{
  "request_id": "00000000-0000-0000-0000-000000000000",
  "project_revision": 13,
  "outcome": {
    "type": "node_deleted",
    "value": {}
  }
}
```

## Atomic command batches

```http
POST /api/v1/projects/{project}/command-batches
POST /api/v1/projects/{project}/command-batches/{batch}/undo
```

Forward request:

```json
{
  "request_id": "00000000-0000-4000-8000-000000000001",
  "expected_revision": 12,
  "commands": [{"type":"delete_node","payload":{"id":"A"}}]
}
```

The server accepts 1 to 100 commands, validates and applies them on an isolated catalog, writes the complete batch ahead, and publishes the catalog once. Any command failure leaves project state and replay unchanged. Each committed `ChangeSet` includes `batch_id`; deterministic child request IDs make exact retries idempotent.

The undo route accepts the same body shape and treats `commands` as an explicit compensation plan. Its `ChangeSet`s include both their new `batch_id` and `compensates` pointing to the selected forward batch. Compensation advances history instead of restoring an old snapshot. Missing, already-compensated, or compensation target batches are rejected.

## Graph reads

```http
GET /api/v1/projects/{project}/nodes
GET /api/v1/projects/{project}/nodes/{entity}
GET /api/v1/projects/{project}/edges
GET /api/v1/projects/{project}/edges/{edge}
GET /api/v1/projects/{project}/estimates?address={estimate_address}
```

Edge and estimate addresses are URL-encoded by clients.

## Project documents

```http
GET /api/v1/projects/{project}/scenarios
GET /api/v1/projects/{project}/scenarios/{scenario}
GET /api/v1/projects/{project}/dependence
GET /api/v1/projects/{project}/formulas
GET /api/v1/projects/{project}/formula?address={component_address}
```

Scenario, formula, and dependence mutations use the command endpoint.

## Structural analysis

```http
GET /api/v1/projects/{project}/analysis/structure
    ?scenario={optional_scenario}
    &maximum_cycle_length=8
    &maximum_cycles=1000
```

The response contains an immutable revision key, exact SCCs, canonical cycles, limits, and a truncation flag.

## Scenario analysis

```http
GET /api/v1/projects/{project}/scenarios/{scenario}/analysis
```

The response contains the immutable graph/scenario/dependence/formula revision key, planning horizon, and independently sampled candidate/objective projections. A `422 scenario_analysis_unavailable` response identifies missing baselines or unsupported non-empty dynamic dependence.

## Change replay

```http
GET /api/v1/projects/{project}/changes?after=12
```

The cursor is exclusive. A cursor newer than the current revision returns `invalid_replay_revision`.

## WebSocket stream

```text
GET ws://host/api/v1/projects/{project}/changes/ws?after=12
```

Protocol messages:

```json
{"type":"change","value":{}}
{"type":"caught_up","value":{"revision":15}}
{"type":"replay_required","value":{"after_revision":15}}
```

See [collaboration and revisions](../guide/collaboration.md) for reconnect semantics.

## Error envelope

Errors use an HTTP status plus a stable machine code, message, and recovery advice:

```json
{
  "error": {
    "code": "project_revision_conflict",
    "message": "project revision conflict: expected 12, current 13",
    "advice": [
      "Refresh the project and retry the command against its current revision."
    ]
  }
}
```

Clients should branch on `code`, not parse the human-readable message.

## Current security boundary

The current API has no authentication or authorisation middleware. Bind to localhost or protect it behind a trusted reverse proxy during development. OIDC roles and production TLS guidance remain planned work.
