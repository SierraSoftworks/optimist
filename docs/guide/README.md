# Getting started

This guide takes a shipped example, asks it the four questions the tool can
answer, and then opens it in the workbench.

## Prerequisites

- Rust 1.96 or newer
- A local checkout of Optimist
- Node 20 or newer, if you want the workbench

```sh
cargo build
```

The examples below use `cargo run --` in place of the built binary. If you have
installed `optimist` onto your path, drop that prefix.

## A design is a directory

There is no database and no server to start. A design is a directory of YAML:

```text
examples/checkout/
  _system.yaml            name, shared quantities, scale units, interventions
  components/browsers.yaml
  components/api.yaml
  components/orders.yaml
```

Each component file declares one component and the relationships leaving it, so
adding a dependency touches one file rather than a shared list that everybody
would have to agree on the ordering of.

## Check what it contains

```sh
cargo run -- check examples/checkout
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

`check` reads and validates the directory without solving it. It is the fastest
way to find a misspelt property or a relationship pointing at a component that
does not exist, and it is what to put in continuous integration.

Six component types and eight behaviours are available even though this design
defines none of its own: they are the shipped catalogue. See
[the catalogue reference](../reference/catalogue.md) for what is in it.

## Solve it

```sh
cargo run -- solve examples/checkout
```

```text
COMPONENT  CHANNEL          VALUE
api        capacity         685.1550 [450.9287 .. 947.7374]
api        hold_time        0.0127 [0.0084 .. 0.0177]
api        utilisation      2.9597 [0.9499 .. 4.9170]
browsers   latency          0.3447 [0.0279 .. 0.5949]
browsers   success          0.7219 [0.4506 .. 1.0000]
orders     volume           3504303674979.1333 [2303453606495.4536 .. 4777574410209.3037]
```

Every quantity is shown as its mean with a central eighty percent interval
beside it. The interval is the point: `service_time` was authored as a
lognormal, so `capacity` is a distribution and the design is congested in some
draws and comfortable in others.

Narrow the report to one component with `--component api`, and raise the draw
count with `--samples 10000` when the tails matter.

## Find what constrains it

```sh
cargo run -- bottlenecks examples/checkout
```

```text
COMPONENT  CONSTRAINT          UTILISATION  P90     BINDS  REPLICAS  HEADROOM
orders     volume              7.009        9.555   100%   1         -3004303674979.1333
api        capacity            2.960        4.916   87%    1         -1063.2349
browsers   success_objective   55.626       109.865 86%    1         -0.2731
browsers   latency_objective   0.460        0.793   3%     1         0.4053
orders     operations          0.066        0.090   0%     1         4669.9294
```

A constraint pairs a demand with the limit it consumes. `UTILISATION` is the mean
of that ratio, `BINDS` is the share of draws in which demand met or exceeded the
limit, and the list is ranked by `BINDS` first. Ranking by probability rather
than by mean puts the constraint most exposed to a bad draw at the top, which is
the one worth spending on.

Add `--binding` to hide everything with headroom in every draw.

Note what the ranking says here. Thirty days of retention overruns the store
several times over, and the objective the client declared is missed in most
draws — but the pool everybody watches is only third on the list.

## Weigh a proposed change

A change is not an edit. An intervention rebinds named quantities in the
scratchpad and the design is solved again exactly as it stands, so whatever moves
in the result moved because of the rebinding.

```yaml
# _system.yaml
interventions:
  - id: warm-cache
    name: Warm the cache
    summary: Raise the hit ratio by holding a larger working set.
    overrides:
      - name: cache_hits
        expression: '0.95'
```

```sh
cargo run -- compare examples/checkout warm-cache
```

```text
COMPONENT  CONSTRAINT          BEFORE  AFTER    BOUND BEFORE  BOUND AFTER  EFFECT
orders     volume              7.009   0.643    100%          0%           relieved
orders     operations          0.066   0.006    0%            0%           eased
browsers   latency_objective   0.460   0.495    3%            8%           loaded
api        capacity            2.960   3.186    87%           87%          loaded
```

Caching relieves the store outright and loads the pool slightly, because the
requests that used to fail at the store now reach it. Relieving one limit
routinely promotes another, and `compare` says so rather than reporting the
improvement alone.

## Open it in the workbench

The commands above read files. To edit a design with other people, serve a
directory of them:

```sh
cargo run -- serve --designs ./designs
```

`designs` is a scratch directory; seed it from `examples/` to start with
something to look at. The server exposes the API on `http://127.0.0.1:3000` and
serves the workbench from the same process when a build is available.

```sh
npm --prefix workbench install
npm --prefix workbench run build
cargo run -- serve --designs ./designs
```

For front-end development, run Vite's dev server instead; it proxies `/api`,
including the WebSocket upgrade the change feed needs.

```sh
npm --prefix workbench run dev     # http://127.0.0.1:5173
```

## Next steps

- [Designing a system](./modelling.md) — components, relationships, signals, behaviours, scale units.
- [Writing component types](./component-types.md) — adding a kind of component the catalogue does not have.
- [Uncertainty](./uncertainty.md) — Squiggle, sample sets, determinism, and the queueing builtins.
- [Solving and bottlenecks](./analysis.md) — how the fixed point is found and what convergence means.
- [The workbench](./collaboration.md) — sessions, mutations, and the change feed.
- [The worked examples](../examples/README.md) — including a design with two steady states.
