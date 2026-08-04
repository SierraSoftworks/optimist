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

Optimist is a workbench, server, and Rust toolkit for designing large systems and
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

Open a design and you get the graph, what each component is sized against, and
whether the whole thing solves — all at once.

![The workbench editing a design: a diagram of three components, the inspector for the selected one, and a green badge saying the design solves.](/screenshots/design.png)

Every field is a Squiggle expression rather than a number. A service time
measured with spread stays a distribution the whole way through the solve, and
the preview beside the field you are typing into shows the spread you just
authored.

![A shared quantity being edited, with a flyout showing the density of the lognormal it evaluates to and its p10, median, and p90.](/screenshots/quantities.png)

Underneath, that design is a directory of small YAML files that belongs in the
same repository as the system it describes — so a capacity question is answered
locally, reviewed in a pull request, and checked by the same continuous
integration that builds the thing being designed.

```yaml
# components/api.yaml
id: api
name: Checkout API
type: compute
properties:
  service_time: lognormal(-4.6, 0.35)
  parallelism: pool_size
```

## Ask it what constrains it

Stop on a component and it says which limit it is closest to exhausting, and by
how much. The colour on the diagram is the same measurement, so a strained
component is visible without opening anything.

![A component in the diagram with a flyout listing its constraints, each with a load bar and an explanation of what saturating it means.](/screenshots/limits.png)

A constraint pairs a demand with the limit it consumes, and the ranking is by the
share of draws in which demand meets or exceeds that limit. Ranking by
probability rather than by mean puts the constraint most exposed to a bad draw at
the top, which is the one worth spending on.

## Weigh a proposed change

A proposal is not an edit. It rebinds named quantities and the design is solved
again exactly as it stands, so the baseline is drawn on the same axes as the
variant and the distance between the two lines is the whole answer.

![The simulation view comparing a variant against the design it would replace, with the baseline drawn dashed and each constraint's movement beside it.](/screenshots/comparison.png)

## Run it

```sh
optimist serve --designs ./designs
```

The server opens a directory of designs, streams every edit over a WebSocket, and
serves the workbench from the same process. There is a
[command-line interface](./reference/cli.md) over the same engine for continuous
integration and scripting. Continue with the
[getting-started guide](./guide/README.md).

::: warning Development status
The modelling, solving, and ranking core is usable today, and the workbench,
HTTP API, and CLI all run against it. Authentication is not implemented. The
on-disk schema is at version two, and version one directories are refused rather
than converted.
:::
