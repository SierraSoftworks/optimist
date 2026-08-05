# Getting started

This guide opens a shipped example in the workbench and asks it the four
questions the tool can answer: what is in this design, what does it do, what
constrains it, and would the proposed change help.

## Prerequisites

- Rust 1.96 or newer
- Node 20 or newer
- A local checkout of Optimist

```sh
npm --prefix workbench install
npm --prefix workbench run build
cargo build --release
```

## Open a workspace

A workspace is a directory whose subdirectories are designs. Point the server at
one and it serves both the API and the workbench from the same process.

```sh
cargo run --release -- serve --designs ./examples
```

Open `http://127.0.0.1:3000` and every design in the directory is listed.

![The workbench's design picker, listing the shipped examples as cards with their summaries.](/screenshots/designs.png)

The rest of this guide uses **Metastable saturation**: a checkout service whose
workers are held for the whole of a downstream call, behind a retry policy. It is
chosen because a summary hides what matters about it.

::: tip Working on the front end
Run Vite's dev server instead. It proxies `/api`, including the WebSocket upgrade
the change feed needs.

```sh
npm --prefix workbench run dev     # http://127.0.0.1:5173
```
:::

## Check what it contains

The **Design** view is the whole model on one screen: the components, what calls
what, the behaviours on each relationship, and the shared quantities everything
is sized against.

![The design view with the checkout service selected, showing its properties, the quantities it derives, and a badge reading "solves".](/screenshots/design.png)

Three things are worth noticing.

- The badge above the diagram says whether the design **solves**. It turns into
  the reason it does not, naming the component the solver blamed, which is the
  fastest way to find a misspelt property or a relationship pointing at a
  component that does not exist.
- The inspector's **properties** are Squiggle source rather than numbers.
  `worker_pool` is a reference to a shared quantity; so is `service_time`, and
  that one is a distribution.
- **Computes** lists the quantities the component's type derives. Nothing there
  was authored; it comes from the shipped catalogue, described in
  [the catalogue reference](../reference/catalogue.md).

Nothing in this design defines a component type of its own. Six types and eight
behaviours are available regardless, and a design may
[define its own](./component-types.md).

## See what it is sized against

Shared quantities sit down the right-hand side. They are what the design is
measured against, and they are the only things a variant can rebind, so anything
you intend to vary belongs here.

![A shared quantity being edited, with a flyout showing the density of the lognormal it evaluates to and its p10, median, and p90.](/screenshots/quantities.png)

The preview appears while a field has focus and shows what the expression being
typed actually evaluates to. `0.05 * lognormal(0, 0.1)` is an inventory lookup
that is usually 50 ms and occasionally 57 ms, and that spread is carried through
the solve rather than averaged away at the start. See
[uncertainty](./uncertainty.md) for the vocabulary.

## Find what constrains it

Components on the diagram are coloured by what they are closest to exhausting.
Stop on one and it says which limit, and by how much.

![A component in the diagram with a flyout listing its constraints, each with a load bar and an explanation of what saturating it means.](/screenshots/limits.png)

A constraint pairs a demand with the limit it consumes. The list is ranked by the
share of draws in which demand met or exceeded that limit, so the top of it is
the resource the design is most exposed to rather than the component somebody
happens to be worried about.

Note what the ranking says here. The objectives the shoppers declared are missed
by two orders of magnitude, and the pool everybody watches is not what the flyout
names first.

## Watch it over time

The **Simulation** view solves the design across a horizon and charts whatever
you choose to watch. The cards along the top are the constraints under the most
pressure, so the limit a chart is about is on the same screen as the chart.

![The simulation view showing success rate and response time collapsing partway through the run, with the four most pressured constraints carded above.](/screenshots/simulation.png)

This is the point of the example. Demand surges between t = 5 and t = 15 and then
returns to a level the design served comfortably before — and the design does not
return with it. Success falls to nothing and stays there. A steady-state answer
would have reported the healthy branch and shown none of this; see
[solving and bottlenecks](./analysis.md) for why.

The shading around each line is the distribution across draws rather than a band
drawn after the fact. Stop on a step and the spread behind that point is drawn
beside it.

## Weigh a proposed change

A change is not an edit. A **variant** rebinds named quantities in the shared
list and the design is solved again exactly as it stands, so whatever moves in
the result moved because of the rebinding.

```yaml
# _system.yaml
interventions:
  - id: shed
    name: Refuse what cannot be served
    summary: Cap admitted demand below the load at which the service saturates.
    overrides:
      - name: admission_limit
        expression: safe_admission
```

Pick one from the left and the design as it stands is drawn on the same axes,
dashed, along with how far every quantity moved.

![The simulation view comparing the load-shedding variant against the design it would replace, with the baseline drawn dashed and each quantity's movement beside it.](/screenshots/comparison.png)

Shedding load is the only one of these variants that keeps the service healthy
through the surge rather than merely surviving it, and it is not free: the
callers it refuses are failures too. The badge on each quantity says how far it
moved, and the card in the header says whether the constraint it was aimed at
still binds.

Relieving one limit routinely promotes another. Comparing rather than re-solving
is what makes that visible.

## Keep it in the repository

Edits are held in memory and written back to the design directory after a short
quiet period. The whole directory is rewritten in canonical form, so a session in
the workbench produces a clean diff that reads as the change that was actually
made.

```text
examples/metastable/
  _system.yaml            name, shared quantities, scale units, variants
  components/shoppers.yaml
  components/checkout.yaml
  components/inventory.yaml
```

Each component file declares one component and the relationships leaving it, so
adding a dependency touches one file rather than a shared list that everybody
would have to agree on the ordering of.

That is what makes a design reviewable: it lives beside the system it describes,
and the same engine that answered the questions above can be run over it in
continuous integration.

```sh
optimist check examples/metastable
```

The [command-line interface](../reference/cli.md) covers everything the workbench
does — `check`, `solve`, `bottlenecks`, `compare` — for the cases where a script
is the right client.

## Next steps

- [Designing a system](./modelling.md) — components, relationships, signals, behaviours, scale units.
- [Writing component types](./component-types.md) — adding a kind of component the catalogue does not have.
- [The expression language](./language.md) — Squiggle syntax, builtins, and the names available inside a design.
- [Choosing distributions](./distributions.md) — what to reach for per input, and sensible starting figures.
- [Uncertainty](./uncertainty.md) — sample sets, determinism, and where spread belongs.
- [Solving and bottlenecks](./analysis.md) — how the fixed point is found and what convergence means.
- [Laws and models](../reference/laws.md) — the queueing, scaling, and reliability laws behind every figure.
- [The workbench](./collaboration.md) — sessions, mutations, and the change feed.
- [The worked examples](../examples/README.md) — including a design with two steady states.
