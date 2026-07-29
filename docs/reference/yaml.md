# Design directory format

A design is a directory rather than a file:

```text
_system.yaml                 name, shared quantities, scale units, interventions
components/<id>.yaml         one component and the relationships leaving it
component-types/<id>.yaml    component types this design defines for itself
mutators/<id>.yaml           behaviours this design defines for itself
```

Splitting components across files is what makes a design reviewable. A model
large enough to be worth building is too large to read as one document, and two
engineers changing different parts of it should not meet in a diff. A
relationship is stored with the component it leaves, so adding a dependency
touches one file.

Quantities a design refers to as a whole stay together in `_system.yaml`. Shared
quantities, scale unit membership, and interventions are statements about the
design rather than about a part of it, and scattering them would mean reading
every file to learn what a model assumes.

## Identifiers

An identifier is used as a file name, so it must match `[a-z0-9_-]{1,128}`.
Upper case, `/`, `\`, `.`, and `..` are all rejected.

## Schema version

The current version is **2**. Version one described the causal graph this tool
was built around before it became a system design tool; the two schemas share no
structure, so a version one directory is refused rather than converted.

Unknown fields are refused too, at every level rather than only at the top of a
document. A file that nearly parses is more dangerous than one that does not:
silently dropping a misspelt property would leave a model quietly using a default
while its author believed otherwise, and every number downstream would look
plausible. The error names the path to the offending key and lists what was
expected there:

```text
_system.yaml: scratchpad[0]: unknown field `unti`,
  expected one of `name`, `expression`, `unit`, `summary` at line 7 column 5
```

## Values are expressions

Every property, capacity, replica count, and override is a **string** holding
Squiggle source, not a YAML number. Quote plain numbers so YAML does not coerce
them:

```yaml
properties:
  parallelism: '8'                    # a constant
  service_time: lognormal(-4.6, 0.35) # a distribution
  replicas: pool_size                 # a shared quantity
```

---

## `_system.yaml`

```yaml
schema_version: 2
name: Checkout
summary: >
  A worked example: browsers reach an API pool that reads from a store, with
  retries in front of the pool and a cache in front of the store.

scratchpad:
  - name: peak_rate
    expression: '900'
    unit: op/s
    summary: Requests per second at the daily peak.
  - name: cache_hits
    expression: '0.5'
    unit: share
    summary: Share of reads served from cache.

scale_units:
  - id: cell
    name: Serving cell
    summary: One self-contained deployment of the serving path.
    replicas: '12'
    distribution: sharded
    members: [api, orders]
    parent: null

interventions:
  - id: warm-cache
    name: Warm the cache
    summary: Raise the hit ratio by holding a larger working set.
    overrides:
      - name: cache_hits
        expression: '0.95'
```

| Field | Required | Notes |
| --- | --- | --- |
| `schema_version` | yes | Must be `2`. |
| `name` | yes | Human-readable name. |
| `summary` | no | What the design is for. |
| `scratchpad` | no | Shared quantities, **in evaluation order**. A later entry may refer to an earlier one. |
| `scale_units` | no | Replication boundaries. |
| `interventions` | no | Proposed changes, as rebindings of scratchpad quantities. |

### Scratchpad entry

| Field | Required | Notes |
| --- | --- | --- |
| `name` | yes | The binding component properties refer to. |
| `expression` | yes | Squiggle source. |
| `unit` | no | Unit annotation, such as `op/s`. Use `share` for a proportion of a whole and `1` for a plain count. |
| `summary` | no | What the quantity is and where its value came from. |

### Scale unit

| Field | Required | Notes |
| --- | --- | --- |
| `id` | yes | Unique within the design. |
| `name` | yes | Human-readable name. |
| `summary` | no | What this boundary represents. |
| `replicas` | yes | Squiggle source for how many exist. |
| `distribution` | no | `sharded` (default) or `mirrored`. |
| `members` | no | Components deployed inside this unit. |
| `parent` | no | The unit enclosing this one. |

A component may be claimed directly by only one unit; nest the units instead.
Units may not enclose each other in a cycle.

### Intervention

| Field | Required | Notes |
| --- | --- | --- |
| `id` | yes | Unique within the design. |
| `name` | yes | Human-readable name. |
| `summary` | no | What the change is and what it would cost to make. |
| `overrides` | no | List of `{ name, expression }`. `name` must be an existing scratchpad entry. |

---

## `components/<id>.yaml`

```yaml
id: browsers
name: Browsers
type: client
properties:
  request_rate: peak_rate
  payload: '512'
  latency_target: '0.75'
  success_target: '0.995'
position:
  x: 40.0
  y: 120.0
outgoing:
  - to: api
    summary: Checkout requests arriving at the API.
    mutators:
      - type: retry
        properties:
          attempts: '3'
      - type: timeout
        properties:
          budget: '1'
```

| Field | Required | Notes |
| --- | --- | --- |
| `id` | yes | Unique within the design, and must match the file name. |
| `name` | yes | Human-readable name. |
| `type` | yes | The component type this instance adopts. |
| `properties` | no | Squiggle source per property the type declares. Every required property must be present. |
| `position` | no | `{ x, y }` diagram layout, written when somebody moves the component. |
| `outgoing` | no | Relationships leaving this component. |

Layout is stored with the design because it carries meaning. Somebody who
arranges a diagram is saying how the system is best read, and that judgement is
worth reviewing alongside the model it describes. It is absent until somebody
moves a component, so an unarranged design is laid out automatically rather than
pinned to whatever an algorithm produced the first time it was opened.

### Outgoing relationship

| Field | Required | Notes |
| --- | --- | --- |
| `to` | yes | The component receiving the flow. Must exist in the design. |
| `from_port` | no | Outbound port on the owning component. Omit when its type declares exactly one. |
| `to_port` | no | Inbound port on the receiving component. Omit when its type declares exactly one. |
| `capacity` | no | Squiggle source for how many operations may wait on the wire. Defaults to `100`. |
| `mutators` | no | Behaviours applied to the flow, **in the order they take effect**. |
| `summary` | no | What this connection represents. |

### Attached behaviour

```yaml
mutators:
  - type: retry
    properties:
      attempts: '3'
```

`type` is the behaviour's identifier; `properties` supplies Squiggle source for
each setting it declares.

---

## `component-types/<id>.yaml`

Component types the design defines for itself, loaded over the shipped catalogue
and validated by identical rules. A definition may replace a catalogue type as
well as add one nobody anticipated.

```yaml
id: token-bucket
name: Token bucket
summary: Admits requests against a refilling allowance.
ports:
  in:
    requests:
      arity: many
      summary: Callers arriving at the limiter.
      publishes:
        success: admitted_ratio
        latency: '0'
  out:
    downstream:
      arity: one
      summary: The service this limiter protects.
      publishes:
        rate: admitted
properties:
  refill:
    unit: op/s
    summary: Tokens added per second.
  burst:
    unit: op
    summary: Tokens the bucket holds.
    default: '0'
channels:
  offered:
    unit: op/s
    summary: Demand arriving from every caller.
    expression: in.requests.rate
  admitted:
    unit: op/s
    summary: Demand passed on, capped per draw at the refill rate.
    expression: min([offered, refill])
  admitted_ratio:
    unit: '1'
    summary: Share of callers served rather than refused.
    expression: min([admitted / max([offered, 0.000001]), 1])
constraints:
  throughput:
    summary: Offered load against the sustained allowance.
    demand: offered
    limit: refill
```

| Section | Notes |
| --- | --- |
| `ports.in` / `ports.out` | Named attachment points. Each has `arity` (`one` or `many`, default `many`), `summary`, and `publishes`: a map of signal name to an expression naming a property or channel. |
| `properties` | Each has `unit`, optional `summary`, and optional `default`. A property without a default must be supplied. |
| `channels` | Each has `unit`, optional `summary`, and `expression`. |
| `constraints` | Each has optional `summary`, `demand`, and `limit`. |

See [writing component types](../guide/component-types.md) for what an expression
may refer to and the rules a definition must satisfy.

---

## `mutators/<id>.yaml`

Behaviours the design defines for itself.

```yaml
id: hedged-request
name: Hedged request
summary: Issues a duplicate once the first request is slow.
properties:
  hedge_share:
    unit: '1'
    summary: Share of calls slow enough to be hedged.
requests:
  rate:
    unit: op/s
    summary: Demand raised by the share of calls that are duplicated.
    expression: signal.rate * (1 + hedge_share)
responses:
  latency:
    unit: s
    summary: Waiting shortened by taking whichever answers first.
    expression: signal.latency * (1 - 0.3 * hedge_share)
```

`requests` rewrites signals travelling downstream; `responses` rewrites them on
the way back. Both accept a map of signal name to `{ unit, summary, expression }`.
`transforms` and `feedback` are accepted as aliases for backward compatibility.

---

## What the loader checks

- The schema version is exactly `2`.
- No document uses an unknown field, at any depth.
- No two components claim the same identifier.
- Every relationship names a component the design contains.
- Every project-local component type and behaviour passes the same validation as
  a shipped one, including the rejection of unknown fields.

Order within the files carries no meaning; a design that has been through
persistence is written back in canonical order — components by identifier,
relationships by endpoint pair, scale units by identifier — so a model assembled
by hand and one assembled by the workbench produce identical files.

Run the checks without solving:

```sh
optimist check ./design
```
