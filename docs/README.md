---
home: true

title: Optimist
titleTemplate: false

heroText: Optimist

tagline: Design large systems and find what constrains them.

actions:
  - text: Get Started
    link: /guide/
  - text: See the Examples
    link: /examples/
    type: secondary

features:
  - title: Component-centric designs
    details: A design is a graph of typed components — clients, load balancers, queues, compute pools, datastores — wired together and annotated with quantities an engineer can measure.
  - title: Component types are data
    details: Properties, channels, ports, and constraints are declared in YAML manifests. Adding a new kind of component means writing a manifest, not changing the engine.
  - title: Uncertainty carried through the solve
    details: Every quantity is a Squiggle expression evaluated over aligned draws, so each draw settles on its own fixed point and the spread of a result is a genuine mixture.
  - title: Bottlenecks, not dashboards
    details: Every constraint pairs a demand with the limit it consumes. The engine ranks them by how likely they are to bind, so the answer names the resource worth spending on.
---

Optimist is a Rust toolkit, server, and workbench for designing large systems and
finding what constrains them. A design is a directory of YAML that belongs in the
same repository as the system it describes, so answering a capacity question is a
local operation and can run in the same continuous integration that builds the
thing being designed.

It exists for the questions a diagram cannot answer:

- Which resource does this design exhaust first, and how sure are we?
- What happens to a pool of workers when the store it calls slows down?
- How much extra demand does that retry policy create once the dependency starts failing?
- Does the design still meet its latency objective at the ninetieth percentile of demand?
- Would the proposed change relieve the binding constraint, or only move it?

## What a design looks like

A shop front — browsers calling an API pool that reads from an order store — is
a directory of small files.

```yaml
# _system.yaml
schema_version: 2
name: Checkout
scratchpad:
  - name: peak_rate
    expression: '900'
    unit: op/s
    summary: Requests per second at the daily peak.
```

```yaml
# components/browsers.yaml
id: browsers
name: Browsers
type: client
properties:
  request_rate: peak_rate
  latency_target: '0.75'
  success_target: '0.995'
outgoing:
  - to: api
    summary: Checkout requests arriving at the API.
    mutators:
      - type: retry
        properties:
          attempts: '3'
```

```yaml
# components/api.yaml
id: api
name: Checkout API
type: compute
properties:
  service_time: lognormal(-4.6, 0.35)
  parallelism: pool_size
```

Every value is a Squiggle expression. A service time measured with spread stays a
distribution the whole way through the solve, and the result says how much of
that distribution has crossed into congestion.

## Ask it something

```sh
optimist check       examples/checkout
optimist solve       examples/checkout
optimist bottlenecks examples/checkout
optimist compare     examples/checkout warm-cache
```

`bottlenecks` is the one worth reading first. It ranks every constraint in the
design by the share of draws in which demand meets or exceeds its limit, so the
top of the list is the resource the design is closest to exhausting rather than
the component somebody happens to be worried about.

## Edit it together

```sh
optimist serve --designs ./designs
```

The server opens a directory of designs, streams every edit over a WebSocket, and
serves the Vue workbench from the same process. Continue with the
[getting-started guide](./guide/README.md).

::: warning Development status
The modelling, solving, and ranking core is usable today, and the CLI, HTTP API,
and workbench all run against it. Authentication is not implemented. The
on-disk schema is at version two, and version one directories are refused rather
than converted.
:::
