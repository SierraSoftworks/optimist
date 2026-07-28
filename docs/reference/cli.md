# CLI reference

```text
optimist [--output <FORMAT>] <COMMAND>
```

> Design large systems and find what constrains them.

There are two things to do with a design: read one from disk and ask it
questions, or serve a directory of them so the workbench and other editors can
work together. Every command is one of those.

## Global options

| Option | Values | Default | Effect |
| --- | --- | --- | --- |
| `--output` | `table`, `json`, `jsonl` | `table` | Report format. Accepted before or after the subcommand. |
| `--version` | | | Print the version and exit. |
| `--help` | | | Print usage and exit. |

`table` output is tab-separated rather than padded, so it pipes into `cut` and
`awk` unchanged and a long name never pushes a column out of alignment.

Errors are rendered with recovery advice and exit with status `1`; success exits
with `0`.

## Shared solve options

`solve`, `bottlenecks`, and `compare` all accept these.

| Option | Default | Effect |
| --- | --- | --- |
| `--seed <U64>` | `0` | Root of the deterministic random stream. |
| `--samples <N>` | `1000` | Draws carried through every uncertain quantity. |
| `--horizon <N>` | `1` | Number of steps to advance. |
| `--step <SECONDS>` | `1.0` | Length of one step. |
| `--transient` | off | Advance queues through time rather than solving for where they balance. |

`--transient` gives a design memory, so a queue filled by a surge has to drain
before the design recovers. It is faithful only while the step is short against
the time a queue takes to empty, so expect to shorten `--step` and lengthen
`--horizon` together.

---

## `check`

```text
optimist check <DIRECTORY>
```

Loads a design and reports what it contains without solving it. This is the
command for continuous integration: it validates the schema version, the
component documents, every relationship endpoint, and every project-local
component type and behaviour definition.

```sh
optimist check examples/checkout
```

```text
PROPERTY        VALUE
name            Checkout
components      3
relationships   2
shared quantities       3
scale units     0
interventions   2
component types 6
behaviours      8
```

`--output json` emits the loaded `SystemModel` — scratchpad, components,
relationships, scale units, and interventions — as one document.

---

## `catalogue`

```text
optimist catalogue <DIRECTORY>
```

Lists the component types and behaviours available to a design, shipped and
project-local together.

```sh
optimist catalogue examples/checkout
```

```text
KIND       ID                    PROPERTIES  LIMITS
component  aggregator            2           1
component  client                4           2
component  compute               4           1
component  datastore             8           4
component  load-balancer         4           2
component  queue                 2           2
behaviour  batch                 2           3
behaviour  cache                 1           1
behaviour  fan-out               1           1
behaviour  feature-flag          1           1
behaviour  ignores-cancellation  0           1
behaviour  load-shed             1           2
behaviour  retry                 1           3
behaviour  timeout               1           3
```

For a `component` row, `LIMITS` is the number of constraints. For a `behaviour`
row it is the number of signals the behaviour rewrites, across both directions.

`--output json` emits the full manifests: ports, properties, channels, and
constraints, with every `summary`.

---

## `solve`

```text
optimist solve <DIRECTORY> [--component <ID>] [--intervention <ID>] [solve options]
```

Solves a design and reports the quantities flowing through it.

```sh
optimist solve examples/checkout --component api
```

```text
COMPONENT  CHANNEL                    VALUE
api        capacity                   685.1550 [450.9287 .. 947.7374]
api        hold_time                  0.0127 [0.0084 .. 0.0177]
api        in.requests.rate           1754.8106 [900.2660 .. 2234.2242]
api        out.dependencies.latency   0.0020 [0.0020 .. 0.0020]
api        utilisation                2.9597 [0.9499 .. 4.9170]
```

Uncertain quantities are shown as a mean with a central eighty percent interval.
Certain ones are shown as a single number. Alongside the component's own
channels, `in.<port>.<signal>` is what arrived and `out.<port>.<signal>` is what
came back.

If the model did not settle, a note follows the table:

```text
Did not settle after 1500 passes; largest movement 3.412e-2.
A loop whose gain exceeds one has no steady state to find.
```

### JSON shape

```json
{
  "api": {
    "capacity": { "mean": 685.155, "p10": 450.9287, "p90": 947.7374, "certain": false },
    "servers": { "mean": 8.0, "p10": 8.0, "p90": 8.0, "certain": true }
  }
}
```

`certain` is `true` for a deterministic value, in which case `mean`, `p10`, and
`p90` are equal.

---

## `bottlenecks`

```text
optimist bottlenecks <DIRECTORY> [--binding] [--intervention <ID>] [solve options]
```

Ranks the constraints a design is closest to exhausting, most likely to bind
first.

```sh
optimist bottlenecks examples/checkout --binding
```

```text
COMPONENT  CONSTRAINT          UTILISATION  P90      BINDS  REPLICAS  HEADROOM
orders     volume              7.009        9.555    100%   1         -3004303674979.1333
api        capacity            2.960        4.916    87%    1         -1063.2349
browsers   success_objective   55.626       109.865  86%    1         -0.2731
browsers   latency_objective   0.460        0.793    3%     1         0.4053
```

| Column | Meaning |
| --- | --- |
| `UTILISATION` | Mean of demand over limit. |
| `P90` | Utilisation at the ninetieth percentile of draws. |
| `BINDS` | Share of draws in which demand met or exceeded the limit. |
| `REPLICAS` | Replicas of this component across every enclosing scale unit; the other figures describe one of them. |
| `HEADROOM` | Mean limit less mean demand, in the constraint's own units. |

`--binding` keeps only constraints that bind in at least one draw.

### JSON shape

```json
[
  {
    "component": "orders",
    "constraint": "volume",
    "summary": "Stored bytes against usable capacity.",
    "replicas": 1.0,
    "utilisation": 7.009,
    "utilisation_p90": 9.555,
    "probability_of_binding": 1.0,
    "headroom": -3004303674979.1333
  }
]
```

`--output jsonl` emits one such object per line.

---

## `compare`

```text
optimist compare <DIRECTORY> <INTERVENTION> [solve options]
```

Weighs a proposed change against the design it would replace, solving both with
the same seed and the same draws.

```sh
optimist compare examples/checkout warm-cache
```

```text
COMPONENT  CONSTRAINT          BEFORE  AFTER  BOUND BEFORE  BOUND AFTER  EFFECT
orders     volume              7.009   0.643  100%          0%           relieved
orders     operations          0.066   0.006  0%            0%           eased
browsers   latency_objective   0.460   0.495  3%            8%           loaded
api        capacity            2.960   3.186  87%           87%          loaded
```

`EFFECT` is one of `relieved`, `introduced`, `eased`, `loaded`, or `unchanged`.
When a change introduces a constraint, a note follows the table saying how many
and why that matters.

### JSON shape

```json
[
  {
    "component": "orders",
    "constraint": "volume",
    "before": 7.009,
    "after": 0.643,
    "bound_before": 1.0,
    "bound_after": 0.0
  }
]
```

---

## `serve`

```text
optimist serve [--bind <ADDR>] [--designs <DIR>] [--web-root <DIR>]
```

Serves a directory of designs to the workbench.

| Option | Environment variable | Default | Effect |
| --- | --- | --- | --- |
| `--bind` | `OPTIMIST_BIND` | `127.0.0.1:3000` | Address requests are accepted on. |
| `--designs` | `OPTIMIST_DESIGNS` | `designs` | Directory holding the designs to serve. |
| `--web-root` | `OPTIMIST_WEB_ROOT` | discovered | A frontend build to serve, overriding whatever the binary would use. |

Loaded designs are checked for settled edits every 100 ms and written back after
a short quiet period; anything outstanding is written on shutdown.

Release builds embed a frontend. Debug builds look for `workbench/dist` beside
the repository. Without a valid web root the server remains API-only. See the
[HTTP API reference](./http-api.md).

---

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | The design could not be read, could not be solved, or the arguments were invalid. |

Failures are printed to stderr with the file or component at fault named, and
with advice on what to do about it.
