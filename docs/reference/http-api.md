# HTTP and WebSocket API

Everything is served under `/api/v1`. Start the server with:

```sh
optimist serve --bind 127.0.0.1:3000 --designs ./designs
```

Reads return the design as it stands; the one write endpoint takes the same
mutations the session applies. There is no revision to send, nothing to retry,
and no conflict to resolve, because a mutation names one entity and the last
writer to touch it wins.

There is no authentication. Bind to loopback or run behind something that
enforces access.

## Errors

Every failure is the same envelope:

```json
{
  "message": "No design goes by 'chekcout'.",
  "advice": "List the workspace to see which designs exist."
}
```

| Status | Cause |
| --- | --- |
| `400 Bad Request` | An identifier could name something outside the workspace. |
| `404 Not Found` | No such design, or no such endpoint. |
| `409 Conflict` | A design already exists, or a mutation refers to something that is not there. |
| `422 Unprocessable Entity` | The design is incomplete or inconsistent and could not be solved. |
| `500 Internal Server Error` | The workspace directory, or a design in it, could not be read or written. |

An unknown `/api/*` path is always a JSON 404 and never falls back to the
workbench, so a mistyped endpoint cannot be read by a client as a malformed
success.

---

## `GET /api/v1/health`

```json
{ "status": "ok", "version": "0.1.0", "designs": 2, "unsaved": 0 }
```

`designs` is the number currently loaded in memory; `unsaved` is how many of
those have edits not yet written back.

---

## `GET /api/v1/designs`

Lists the workspace. Reads only each design's header, so it stays cheap as
designs grow.

```json
[
  { "id": "checkout", "name": "Checkout", "summary": "A worked example." },
  { "id": "broken", "name": "", "summary": "", "unreadable": "components/api.yaml: unknown field `parallelisim`" }
]
```

`unreadable` is present only when the design could not be read. It is listed
rather than hidden so that a malformed file is discoverable.

---

## `POST /api/v1/designs`

Starts an empty design. Responds `201 Created` with a snapshot.

```json
{ "id": "payments", "name": "Payments", "summary": "Card capture and settlement." }
```

`name` falls back to `id` when empty, and `summary` defaults to empty. The
identifier becomes a directory name and is checked against the same rule that
guards every other path the server builds: lower-case letters, digits, hyphens,
and underscores, 1–128 characters.

A design with no components is valid. It is what somebody has after naming the
thing they are about to model, and refusing to store it would mean the first edit
had to carry the creation too.

---

## `GET /api/v1/designs/{design}`

Returns a snapshot.

```json
{
  "name": "Checkout",
  "summary": "A worked example.",
  "sequence": 41,
  "model": {
    "scratchpad": [ { "name": "peak_rate", "expression": "900", "unit": "op/s", "summary": "" } ],
    "components": [ { "id": "api", "name": "Checkout API", "type": "compute", "properties": {} } ],
    "relationships": [ { "from": "browsers", "to": "api", "mutators": [], "summary": "" } ],
    "scale_units": [],
    "interventions": []
  }
}
```

`sequence` is the design's position in its change feed. It increments on every
applied mutation.

---

## `GET /api/v1/designs/{design}/catalogue`

Everything a design may draw on.

```json
{
  "component_types": { "compute": { "id": "compute", "name": "Compute", "ports": {}, "properties": {}, "channels": {}, "constraints": {} } },
  "mutators": { "retry": { "id": "retry", "name": "Retry", "properties": {}, "requests": {}, "responses": {} } },
  "signals": { "rate": { "unit": "op/s", "summary": "", "aggregate": "sum", "extensive": true } },
  "builtins": ["Little.rate", "Queue.mmcWait", "Reliability.retryAttempts"]
}
```

`signals` is here because a port publishes signals rather than channels, so a
client showing what arrived at a component has no component type to read a unit
from. `builtins` is every name an expression may call, sent so an editor can
complete what somebody is typing against the vocabulary the server will actually
evaluate.

---

## `POST /api/v1/designs/{design}/mutations` {#mutations}

Applies a batch of changes in order.

```json
{
  "mutations": [
    { "kind": "set_scratchpad_entry", "entry": { "name": "peak_rate", "expression": "1200", "unit": "op/s", "summary": "" } },
    { "kind": "remove_component", "id": "legacy-api" }
  ]
}
```

```json
{ "sequence": 43, "applied": 2 }
```

Each mutation is applied atomically. The batch stops at the first failure, and
earlier mutations stand; `applied` says how many landed.

### Mutation kinds

Every mutation is tagged with `kind`. Unknown fields are rejected, in the
envelope and in the entity it carries, so a client sending a field this server
does not know about is told rather than having it silently dropped.

| `kind` | Payload |
| --- | --- |
| `set_scratchpad_entry` | `entry`: a scratchpad entry. Replaces the one with the same `name`. |
| `remove_scratchpad_entry` | `name` |
| `set_component` | `component`. Replaces the one with the same `id`. |
| `remove_component` | `id`. Also removes every relationship touching it. |
| `set_relationship` | `relationship`. Replaces the one between the same two components. |
| `remove_relationship` | `from`, `to` |
| `set_scale_unit` | `scale_unit`. Replaces the one with the same `id`. |
| `remove_scale_unit` | `id` |
| `set_intervention` | `intervention`. Replaces the one with the same `id`. |
| `remove_intervention` | `id` |

### Entity shapes

**Scratchpad entry**

```json
{ "name": "peak_rate", "expression": "900", "unit": "op/s", "summary": "Requests per second at peak." }
```

**Component** — `type` is the component type it adopts; `position` is optional
diagram layout.

```json
{
  "id": "api",
  "name": "Checkout API",
  "type": "compute",
  "properties": { "service_time": "lognormal(-4.6, 0.35)", "parallelism": "pool_size" },
  "position": { "x": 220.0, "y": 80.0 }
}
```

**Relationship** — `from_port` and `to_port` may be omitted when the type
declares exactly one port on that side. `capacity` defaults to `"100"`.

```json
{
  "from": "browsers",
  "to": "api",
  "from_port": "calls",
  "to_port": "requests",
  "capacity": "100",
  "mutators": [ { "type": "retry", "properties": { "attempts": "3" } } ],
  "summary": "Checkout requests arriving at the API."
}
```

**Scale unit** — `distribution` is `sharded` or `mirrored`.

```json
{
  "id": "cell",
  "name": "Serving cell",
  "summary": "",
  "replicas": "12",
  "distribution": "sharded",
  "members": ["api", "orders"],
  "parent": null
}
```

**Intervention**

```json
{
  "id": "warm-cache",
  "name": "Warm the cache",
  "summary": "Raise the hit ratio by holding a larger working set.",
  "overrides": [ { "name": "cache_hits", "expression": "0.95" } ]
}
```

### What is rejected

A mutation that would break the design structurally returns `409 Conflict`: a
relationship naming a component that does not exist, a relationship that leaves
and arrives at the same component, a scale unit claiming a component another
already claims, a scale unit enclosed by one that does not exist, or a removal of
something that is not there.

A design that is merely *incomplete* is accepted. A component missing a required
property is stored, because that is what somebody has halfway through an edit; it
fails when the design is solved, which is where the message belongs.

---

## `GET /api/v1/designs/{design}/analysis`

Solves the design and ranks its constraints.

| Query parameter | Default | Notes |
| --- | --- | --- |
| `seed` | `0` | |
| `samples` | `1000` | Clamped to 64–20,000. |
| `horizon` | `1` | Clamped to 1–500. |
| `step` | `1.0` | Seconds. |
| `transient` | `false` | Advance queues through time rather than solving for balance. |
| `series` | `false` | Return every step rather than only the one it settled on. |
| `intervention` | none | Apply an intervention before solving. |

Draw count is the one control that costs the server rather than the caller, which
is why it is capped. Solving runs on the blocking pool, so a model that takes a
moment delays only the client that asked for it.

```json
{
  "sequence": 41,
  "converged": true,
  "iterations": 789,
  "components": {
    "api": {
      "capacity": { "mean": 685.155, "p10": 450.93, "p50": 672.1, "p90": 947.74, "draws": [] },
      "in.requests.rate": { "mean": 1754.81, "p10": 900.27, "p50": 1730.4, "p90": 2234.22, "draws": [] }
    }
  },
  "bottlenecks": [
    {
      "component": "orders",
      "constraint": "volume",
      "summary": "Stored bytes against usable capacity.",
      "replicas": 1.0,
      "utilisation": 7.009,
      "utilisation_p90": 9.555,
      "probability_of_binding": 1.0,
      "headroom": -3004303674979.13
    }
  ],
  "series": null
}
```

Channel keys are dotted paths: a component's own channel names, plus
`in.<port>.<signal>` for what arrived and `out.<port>.<signal>` for what came
back.

`draws` carries a subsample of the underlying values — up to 256 per quantity,
and up to 96 per quantity inside a series frame — so a client can draw a density
rather than a summary. It is empty for a certain quantity.

With `series=true`, `series` is an array of frames, each with `time`,
`converged`, and its own `components` map.

`converged` is a claim about every step of the horizon, while `iterations`
belongs to the last one. Where they differ — a design that collapsed under a
surge and settled again once it passed — `moving` describes the step that settled
worst, and is omitted when every step settled:

```json
"moving": {
  "time": 5.0,
  "iterations": 256,
  "component": "browsers",
  "channel": "failure",
  "movement": 0.9013,
  "stalled": true
}
```

`stalled` distinguishes an iterate that stopped getting closer to a steady state
from one that merely ran out of passes; only the second is worth more passes.

`mixed` appears instead where the design settled on several states rather than
one, giving the `time`, `component` and `channel` involved and how many `states`
its draws divided between.

`sequence` says which state of the design this analysis reflects, so a client can
tell whether an answer predates an edit it has already seen.

---

## `GET /api/v1/designs/{design}/comparisons/{intervention}`

Solves the design twice with the same controls, once as it stands and once with
the intervention applied. Accepts the same query parameters as `analysis`.

```json
{
  "baseline": [],
  "proposed": [],
  "movements": [
    {
      "component": "orders",
      "constraint": "volume",
      "before": 7.009,
      "after": 0.643,
      "bound_before": 1.0,
      "bound_after": 0.0
    }
  ]
}
```

`baseline` and `proposed` are full bottleneck lists. `movements` pairs them per
constraint, largest improvement first.

---

## `GET /api/v1/designs/{design}/feed`

A WebSocket. Every message is JSON tagged with `type`.

The socket sends the design as its first message and streams changes after it,
which has no gap to reason about and costs one round trip rather than two.

```json
{ "type": "snapshot", "name": "Checkout", "summary": "", "sequence": 41, "model": {} }
```

```json
{ "type": "change", "sequence": 42, "mutation": { "kind": "set_component", "component": {} } }
```

```json
{ "type": "lagged", "missed": 312 }
```

A socket opened at sequence *N* receives the snapshot at *N* and then every
change with a sequence above it. A client that already has a change can
recognise it by sequence and ignore it — which is how an editor handles its own
edits arriving back.

`lagged` means the listener fell more than 256 changes behind and the backlog was
dropped. That is the one case where refetching the design is the right answer.

---

## Static files

Requests that no API route claims fall through to the workbench.

- A path whose final segment contains a `.` is served as a file, or 404s.
- Anything else is served `index.html`, so browser-side routing works.
- `assets/*` is served `Cache-Control: public, max-age=31536000, immutable`.
- Everything else, including `index.html`, is served `no-cache`.

Path traversal is rejected component by component before any path is joined.
