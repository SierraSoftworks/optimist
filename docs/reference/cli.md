# CLI reference

```text
optimist [--output <FORMAT>] [--colour <WHEN>] [--progress <WHEN>] [COMMAND]
```

> Design large systems and find what constrains them.

Most of this is also in the workbench, and the workbench is the better place to
explore a design. The CLI is for the cases where a script is the right client:
validating a design in continuous integration, producing a machine-readable
answer, and serving the workbench in the first place.

There are three things to do with a design: open the workbench over a folder of
them, serve that folder so the workbench and other editors can work together, or
read one from disk and ask it questions. Every command is one of those, and
running `optimist` with nothing to say does the first.

The questions come in an order, and it is worth following:

| Question | Command |
| --- | --- |
| Does this design mean what I wrote? | `optimist check` |
| What flows through it? | `optimist solve` |
| What does it run out of first? | `optimist bottlenecks` |
| Does this proposal help? | `optimist compare` |
| What can I build it from? | `optimist catalogue` |
| How do I send this to somebody? | `optimist export`, `optimist import` |

Every design command takes the design directory as its first argument and
defaults to the working directory, so somebody standing in a design can ask
anything of it without naming it again. The one exception is `compare`, which
must be given the directory because the arguments after it are interventions.

## Global options

| Option | Values | Default | Effect |
| --- | --- | --- | --- |
| `--output` | `table`, `json`, `jsonl` | `table` | Report format. Accepted before or after the subcommand. |
| `--colour` | `auto`, `always`, `never` | `auto` | Whether reports are coloured. `auto` colours a terminal and nothing else. |
| `--progress` | `auto`, `always`, `never` | `auto` | Whether solve progress is drawn on standard error. `auto` draws only on a terminal. |
| `--version` | | | Print the version and exit. |
| `--help` | | | Print usage and exit. |

`table` output is laid out for a terminal: rounded sections, aligned columns,
and a closing note saying what the numbers mean. Colour is emphasis only —
every figure it highlights is also written out — so a report captured to a
file, read aloud, or parsed by an agent loses nothing. Width is taken from
`COLUMNS` when that is set and from the terminal otherwise, which is how a
script pins a report to a known width.

Use `--output json` when something other than a person is reading. Runtime and
design errors are rendered with recovery advice and exit with status `1`;
invalid command-line arguments exit with status `2`. Success exits with `0`.

## Shared solve options

`solve`, `bottlenecks`, and `compare` all accept these.

| Option | Default | Effect |
| --- | --- | --- |
| `--seed <U64>` | `0` | Root of the deterministic random stream. |
| `--samples <N>` | `1000` | Draws carried through every uncertain quantity. |
| `--horizon <N>` | `1` | Number of steps to advance. |
| `--step <LENGTH>` | `1.0` | Length of one step: how far `t` advances each step, and how far a transient integration carries the backlog. |
| `--transient` | off | Advance queues through time rather than solving for where they balance. |
| `--shares <N>` | `4` | Divide draws into independently solved pieces. The result is exact; values do not depend on the number of shares. |

`--transient` gives a design memory, so a queue filled by a surge has to drain
before the design recovers. It is faithful only while the step is short against
the time a queue takes to empty, so expect to shorten `--step` and lengthen
`--horizon` together.

---

## `check`

```text
optimist check [DIRECTORY] [--no-solve]
```

Loads a design, looks it over, and reports anything wrong with it. This is the
command for continuous integration.

Reading a design already rejects anything malformed: a wrong schema version, a
relationship pointing at nothing, a component type that does not validate. What
`check` adds is everything that parses and still does not say what its author
meant, because the engine absorbs each of those silently and then answers a
question nobody asked.

| Finding | Severity | Why it matters |
| --- | --- | --- |
| A component adopts an unknown type | `error` | Nothing can be derived from it. |
| A component omits a property its type requires | `error` | The design cannot be solved. |
| A component sets a property its type does not declare | `warning` | The value is ignored; usually a misspelling. |
| A relationship attaches an unknown behaviour | `error` | The rewrite it names never happens. |
| A scale unit groups a component that does not exist | `error` | The replication it describes is not applied. |
| An intervention rebinds something outside the scratchpad | `error` | It compares identically against the design. |
| An intervention rebinds nothing at all | `warning` | The same, and usually unfinished. |
| A component is wired to nothing | `warning` | It neither offers nor receives load. |
| The design will not solve | `error` | Reported with the expression at fault. |
| The design does not settle | `warning` | A loop whose gain exceeds one has no steady state. |

The last two come from solving the design once at sixty-four draws. That trial
is skipped when a structural error has already been found, because a design
missing a required property will certainly fail to solve and saying so twice
buries the finding worth acting on. Pass `--no-solve` to check the structure
alone, which is faster and enough when the design is known to solve.

```sh
optimist check examples/checkout
```

```text
╭─ Checkout ───────────────────────────────────────────────────────────────────╮
│ A worked example: browsers reach an API pool that reads from a store, with    │
│ retries in front of the pool and a cache in front of the store.               │
│                                                                              │
│ 3 components, 2 relationships, 3 shared quantities, 0 scale units, 2          │
│ interventions.                                                               │
╰──────────────────────────────────────────────────────────────────────────────╯

╭─ Components ─────────────────────────────────────────────────────────────────╮
│ ID        TYPE       NAME          CALLS  CALLERS  BEHAVIOURS                 │
│ ────────  ─────────  ────────────  ─────  ───────  ──────────                 │
│ api       compute    Checkout API      1        1           1                 │
│ browsers  client     Users             1        0           2                 │
│ orders    datastore  Order store       0        1           0                 │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Sections for shared quantities and interventions follow, and then either the
findings or a note saying there are none.

**The exit status is the point.** A design with any `error` finding exits `1`;
warnings alone exit `0`. The report goes to stdout either way, so it can be read
and acted on whichever it was.

### JSON shape

```json
{
  "name": "Checkout",
  "summary": "A worked example: ...",
  "components": 3,
  "relationships": 2,
  "shared_quantities": 3,
  "scale_units": 0,
  "interventions": 2,
  "solvable": true,
  "findings": [
    {
      "severity": "error",
      "subject": "api",
      "message": "does not supply `service_time`, which `compute` requires (s)",
      "advice": "Give the property a value, or a default in the component type ..."
    }
  ]
}
```

`severity` is `error` or `warning`. `solvable` is `false` when any finding is an
error, which is the same condition as the exit status.

---

## `catalogue`

```text
optimist catalogue [DIRECTORY] [--type <ID>]
```

Lists the component types and behaviours available to a design, shipped and
project-local together, with a count of how many components use each.

```sh
optimist catalogue examples/checkout
```

```text
╭─ Component types ────────────────────────────────────────────────────────────╮
│ TYPE           NAME           PROPERTIES  CHANNELS  LIMITS  IN USE           │
│ ─────────────  ─────────────  ──────────  ────────  ──────  ──────           │
│ aggregator     Aggregator              2        10       1       0           │
│ client         Client                  4         4       2       1           │
│ compute        Compute                 3        17       1       1           │
│ datastore      Datastore               8        13       4       1           │
│ failover       Failover                4        19       0       0           │
│ load-balancer  Load balancer           2        11       1       0           │
│ queue          Queue                   2         6       2       0           │
│ quorum         Quorum                  1        14       1       0           │
╰──────────────────────────────────────────────────────────────────────────────╯

╭─ Behaviours ─────────────────────────────────────────────────────────────────╮
│ BEHAVIOUR              NAME                  PROPERTIES  REQUESTS  RESPONSES │
│ ─────────────────────  ────────────────────  ──────────  ────────  ───────── │
│ batch                  Batch                          2         2          1 │
│ cache                  Cache                          1         1          0 │
│ cancellation-effecti…  Cancellation effect…           1         1          0 │
│ fallible               Fallible link                  2         1          1 │
│ fan-out                Fan-out                        1         2          0 │
│ feature-flag           Feature flag                   1         1          0 │
│ ignores-cancellation   Ignores cancellation           0         1          0 │
│ load-shed              Load shedding                  1         1          1 │
│ message-size           Message size                   2         1          1 │
│ retry                  Retry                          1         1          2 │
│ timeout                Timeout                        1         1          2 │
╰──────────────────────────────────────────────────────────────────────────────╯
```

`--type <ID>` describes one component type or behaviour in full: what it is for,
the properties it expects and which of them are required, the ports it attaches
by, the quantities it derives, and the limits it can exhaust. This is the
reference to reach for when writing a component rather than reading one.

```sh
optimist catalogue examples/checkout --type queue
```

`--output json` emits the full manifests, or the single named one.

---

## `solve`

```text
optimist solve [DIRECTORY] [-c|--component <ID>] [-i|--intervention <ID>] [solve options]
```

Solves a design and reports the quantities flowing through it, a section per
component.

```sh
optimist solve examples/checkout --component api
```

```text
╭─ api ──────────────────────────────────────────────────────────╮
│ CHANNEL                             MEAN          80% INTERVAL │
│ ─────────────────────────────  ─────────  ──────────────────── │
│ capacity                         685.155  450.9287 .. 947.7374 │
│ hold_time                         0.0127      0.0084 .. 0.0177 │
│ utilisation                       2.9597       0.9499 .. 4.917 │
│ in.requests.rate               1754.8106  900.266 .. 2234.2242 │
│ out.dependencies.latency           0.002        0.002 .. 0.002 │
╰────────────────────────────────────────────────────────────────╯
```

Uncertain quantities carry a central eighty percent interval; certain ones do
not. A component's own channels come first, then the traffic on its ports:
`in.<port>.<signal>` is what arrived and `out.<port>.<signal>` is what came
back, so the dependency latency that caused a component's own latency sits in
the same table.

If the model did not settle, a note follows saying so, because a design with no
steady state has no figures worth reading. It names the quantity that was still
moving, which is what turns "nothing settled" into somewhere to look.

If instead the model settled on several states, a note says how many and which
quantity divided between them. The figures are then real, but every mean among
them is taken across the branches and describes none of them.

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
optimist bottlenecks [DIRECTORY] [--binding] [-c|--component <ID>] [-i|--intervention <ID>] [solve options]
```

Ranks the constraints a design is closest to exhausting, most likely to bind
first.

```sh
optimist bottlenecks examples/checkout --binding
```

```text
╭─ Constraints ────────────────────────────────────────────────────────────────╮
│ COMPONENT  CONSTRAINT     LOAD             MEAN       P90  BINDS    HEADROOM │
│ ─────────  ─────────────  ────────────  ───────  ────────  ─────  ────────── │
│ orders     volume         ████████████     7.01      9.56   100%   -3.004e12 │
│ api        capacity       ████████████     2.96      4.92    87%  -1063.2349 │
│ browsers   success_obje…  ████████████    55.63       110    86%     -0.2731 │
│ browsers   latency_obje…  ██████░░░░░░   0.4597    0.7931     3%      0.4053 │
╰──────────────────────────────────────────────────────────────────────────────╯

╭─ orders.volume runs out first ───────────────────────────────────────────────╮
│ It is carrying 7.01× what its limit allows on average and binds in 100% of   │
│ draws. Resident bytes against usable capacity. Unlike the rate limits this   │
│ one fills gradually and then fails abruptly, so headroom here is measured in │
│ time rather than in load.                                                    │
╰──────────────────────────────────────────────────────────────────────────────╯
```

| Column | Meaning |
| --- | --- |
| `LOAD` | Mean utilisation drawn as a bar, filled completely at or beyond the limit. |
| `MEAN` | Mean of demand over limit. |
| `P90` | Utilisation at the ninetieth percentile of draws. |
| `BINDS` | Share of draws in which demand met or exceeded the limit. |
| `HEADROOM` | Mean limit less mean demand, in the constraint's own units. |
| `REPLICAS` | Replicas of this component across every enclosing scale unit. Shown only where a design has any; the other figures describe one replica. |

`--binding` keeps only constraints that bind in at least one draw, and
`--component` keeps only one component's.

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
optimist compare <DIRECTORY> <INTERVENTION>... [solve options]
```

Weighs proposals against the design they would replace, solving each with the
same seed and the same draws. Name several and each is compared independently
with the same baseline; the command does not compare interventions pairwise.

```sh
optimist compare examples/checkout warm-cache bigger-pool
```

```text
╭─ warm-cache ────────────────────────────────────────────────────────────────╮
│ COMPONENT  CONSTRAINT               UTILISATION      BINDS  EFFECT          │
│ ─────────  ─────────────────  ─────────────────  ─────────  ─────────       │
│ orders     volume                 7.01 → 0.6433  100% → 0%  relieved        │
│ orders     operations            0.066 → 0.0061    0% → 0%  eased           │
│ browsers   latency_objective    0.4597 → 0.4955    3% → 8%  loaded          │
╰─────────────────────────────────────────────────────────────────────────────╯

╭─ warm-cache relieves what it was aimed at ──────────────────────────────────╮
│ It stops 1 constraint binding and starts none: orders.volume. 3 constraints │
│ are still binding afterwards: browsers.latency_objective, api.capacity,     │
│ browsers.success_objective.                                                 │
╰─────────────────────────────────────────────────────────────────────────────╯
```

`EFFECT` is one of `relieved`, `introduced`, `eased`, `loaded`, or `unchanged`.
The note beneath each proposal says which of three things it did: relieved what
it was aimed at, moved the bottleneck somewhere else, or changed nothing that
binds. It also names whatever is still binding afterwards, because a change can
relieve one limit and leave the design just as short as it was.

### JSON shape

```json
[
  {
    "intervention": "warm-cache",
    "component": "orders",
    "constraint": "volume",
    "before": 7.009,
    "after": 0.643,
    "bound_before": 1.0,
    "bound_after": 0.0,
    "relieved": true,
    "introduced": false
  }
]
```

Every movement names the proposal it belongs to, so several proposals share one
flat list. `--output jsonl` emits one per line.

---

## `app`

```text
optimist app [--designs <DIR>]
```

Opens the workbench in a window. This is what running `optimist` with no
arguments does, so a packaged application starts here when it is launched from a
desktop rather than a terminal.

| Option | Environment variable | Default | Effect |
| --- | --- | --- | --- |
| `--designs` | `OPTIMIST_DESIGNS` | remembered, else `~/Documents/optimist` | Directory holding the designs to open. |

The first launch says where designs are going and offers somewhere else to put
them; the answer is remembered, and the folder shown beside the title changes it
again at any time. Changing it writes out whatever the old folder still held,
stops watching it, and opens the new one in place, so nothing has to restart.
Passing `--designs` opens that folder for this launch without changing what is
remembered.

The answer is kept in `settings.json` under the platform's configuration
directory — `%APPDATA%\optimist` on Windows, `~/Library/Application
Support/optimist` on macOS, and `$XDG_CONFIG_HOME/optimist` on Linux. Deleting
it makes the next launch ask again.

There is no server and no port. The window reaches the same handlers `serve`
puts behind HTTP through Tauri's IPC, which nothing outside the process can
reach, so a design open in the application is not exposed to anything else
running on the machine.

Only builds made with the `desktop` feature have a window. Everything else in
this reference works the same in either.

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

A released binary embeds a frontend. A debug build from a checkout looks for
`workbench/dist` beside the repository. Without a valid web root the server
remains API-only. See the [HTTP API reference](./http-api.md).

---

## `export`

```text
optimist export [DIRECTORY] [ARCHIVE]
```

Packs a design into a zip file that can be attached to a review, committed
beside a proposal, or sent to somebody who has never run this tool. `ARCHIVE`
defaults to `<directory-name>.zip` in the working directory; `-` writes the
archive to standard output.

Only the documents a design is made of are packed — `_system.yaml`,
`components/`, `component-types/`, and `mutators/` — so editor backups and
version control metadata in the same directory stay where they are. Timestamps
are fixed, so packing an unchanged design twice produces identical bytes and a
checksum means something.

```console
$ optimist export ./checkout
$ optimist export ./checkout - | sha256sum
```

---

## `import`

```text
optimist import <ARCHIVE> [DIRECTORY] [--force]
```

Unpacks a shared archive into a design directory. `DIRECTORY` defaults to the
archive's own name. Importing over an existing design is refused unless
`--force` says otherwise.

The archive is treated as hostile, because by the time it arrives it has been
through at least one system nobody controls. It is unpacked and loaded in full
into a scratch directory before the destination is touched, so a file that turns
out not to be a design leaves whatever was there alone.

| Refused because | What is reported |
| --- | --- |
| It is not a readable zip | `this file is not a readable archive` |
| It holds no `_system.yaml` | `this archive contains no _system.yaml, so it is not a design` |
| A document sits outside the design layout | `'<entry>' is not part of a design` |
| It expands far beyond any design | `this archive unpacks to more than <N> bytes` |
| It was written by another schema | `this design uses schema version <N>, and this build reads version 2` |

Every refusal is printed with advice on what to do about it, and names whether
the sender or the recipient is the one who can fix it.

---

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | The design could not be read, could not be solved, or carried an error-level finding. |
| `2` | The command-line arguments were invalid. |

Failures are printed to stderr with the file or component at fault named, and
with advice on what to do about it. Reports always go to stdout.
