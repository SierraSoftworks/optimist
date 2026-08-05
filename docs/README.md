---
home: true

title: Optimist
titleTemplate: false

heroText: Optimist

tagline: The system design tool you never knew you needed.

actions:
  - text: Get Started
    link: /guide/
  - text: See the Examples
    link: /examples/
    type: secondary

features:
  - title: Simulate designs before you build them
    details: |
      Draw the system you're planning, put the numbers you actually know against it, and find out whether it works —
      before anybody writes the code, provisions the fleet, or signs the cloud bill.

  - title: Model complex behaviour under load
    details: |
      Retries, timeouts, caches, fan-out, queues, and shedding all change how much work arrives downstream. Optimist
      follows those loops through to their fixed point, including the ones that don't have a happy one.

  - title: Find your bottlenecks first
    details: |
      Every limit in the design is ranked by how likely it is to bind, so the answer names the resource worth spending
      on rather than the component somebody happens to be worried about.
---

Optimist is a system design tool. You describe the system you're planning as a
graph of components — clients, load balancers, queues, worker pools, datastores —
annotate it with the quantities you can actually measure, and it tells you what
the design does, where it breaks, and what to fix first.

It exists for the questions a diagram cannot answer:

- Which resource does this design exhaust first, and how sure are we?
- What happens to a pool of workers when the store it calls slows down?
- How much extra demand does that retry policy create once the dependency starts failing?
- Does the design still meet its latency objective at the ninetieth percentile of demand?
- Would the proposed change relieve the binding constraint, or only move it?

::: tip It isn't guessing
Nothing here is a hand-wavy simulation. Capacity comes from Little's Law, waiting
time from M/M/1, M/M/c, and Erlang C, buffers from bounded M/M/1/K queues, and
retry amplification and quorum availability from their closed forms — with
uncertainty carried through all of it as real distributions rather than averages.
Every figure traces back to a named law, and
[Laws and models](./reference/laws.md) says which, where, and under what
assumptions.
:::

## Features

- **Simulate a design before you build it.** A design is a directory of YAML that lives beside the system it describes, so a capacity question is answered locally, reviewed in a pull request, and checked by the same CI that builds the real thing.
- **Model behaviour under load, not just at rest.** Solve for the steady state, or advance queues through time to find out whether an incident ends when its cause does.
- **Rank bottlenecks by risk.** Constraints are ordered by the share of draws in which demand meets or exceeds the limit, per scale unit, so a variable constraint outranks a merely busy one.
- **Weigh a proposal properly.** An intervention rebinds named quantities and the design is solved again exactly as it stands, on the same draws, so the distance between the two lines is the whole answer.
- **Carry uncertainty end to end.** Every quantity is a [Squiggle expression](./guide/language.md), so a service time you measured with spread stays a distribution the whole way through.
- **Extend the vocabulary.** Component types are data. Adding a kind of component means writing a YAML manifest, not changing the engine.

## What a design looks like

Open a design and you get the graph, what each component is sized against, and
whether the whole thing solves — all at once.

![The workbench editing a design: a diagram of three components, the inspector for the selected one, and a green badge saying the design solves.](/screenshots/design.png)

Underneath, that's a directory of small YAML files that lives in the same
repository as the system it describes:

```yaml
# components/api.yaml
id: api
name: Checkout API
type: compute
properties:
  service_time: lognormal(-4.6, 0.35)
  parallelism: pool_size
```

Every field is an expression rather than a number, so a quantity you're unsure
about gets to say so. The preview beside the field you're typing into shows the
spread you just authored.

![A shared quantity being edited, with a flyout showing the density of the lognormal it evaluates to and its p10, median, and p90.](/screenshots/quantities.png)

::: tip Not sure which distribution to reach for?
[Choosing distributions](./guide/distributions.md) covers what to use for request
volume, latency, payload size, hit ratios and the rest, along with
order-of-magnitude starting figures to put in them.
:::

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

## Install

```sh
brew install sierrasoftworks/tap/optimist
```

Or download a binary for Windows, Linux, or macOS in `amd64` or `arm64` from the
[latest release](https://github.com/SierraSoftworks/optimist/releases/latest).

## Example

```sh
# Serve a directory of designs, with the workbench on http://127.0.0.1:3000
optimist serve --designs ./designs

# Load and validate a design without solving it — the CI-friendly check
optimist check examples/checkout

# See what the design does
optimist solve examples/checkout

# See what it runs out of first
optimist bottlenecks examples/checkout

# See whether a proposal would help
optimist compare examples/checkout warm-cache
```

```text
COMPONENT  CONSTRAINT          UTILISATION  P90      BINDS  REPLICAS  HEADROOM
orders     volume              7.009        9.555    100%   1         -3004303674979.1333
api        capacity            2.960        4.916    87%    1         -1063.2349
browsers   success_objective   55.626       109.865  86%    1         -0.2731
browsers   latency_objective   0.460        0.793    3%     1         0.4053
```

The server streams every edit over a WebSocket and serves the workbench from the
same process, and the same engine is available from a script for continuous
integration. Use `--output json` or `--output jsonl` when a script is the client,
and `--seed`, `--samples`, `--horizon`, `--step`, and `--transient` to control the
solve — the [CLI reference](./reference/cli.md) covers the lot.

When you're ready, start with the [getting-started guide](./guide/README.md).

::: warning Development status
The modelling, solving, and ranking core is usable today, and the workbench,
HTTP API, and CLI all run against it. Authentication is not implemented. The
on-disk schema is at version two, and version one directories are refused rather
than converted.
:::

<ClientOnly>
    <Contributors repo="SierraSoftworks/optimist" />
    <Releases repo="SierraSoftworks/optimist" />
</ClientOnly>
