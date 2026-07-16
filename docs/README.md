---
home: true

title: Optimist
titleTemplate: false

heroText: Optimist

tagline: Model complex systems without throwing away uncertainty.

actions:
  - text: Get Started
    link: /guide/
  - text: See the Examples
    link: /examples/
    type: secondary

features:
  - title: Typed systems models
    details: Outcomes, metrics, factors, interventions, and validated relationships keep the graph meaningful and machine-readable.
  - title: Probability with guardrails
    details: Dimensioned estimates, Bayesian updates, Fermi formulas, Gaussian copulas, and deterministic Monte Carlo preserve assumptions and uncertainty.
  - title: Feedback and collaboration
    details: Exact structural loop discovery, revision-checked commands, ordered ChangeSet replay, and WebSocket streams support shared modelling workflows.
---

Optimist is a Rust toolkit and server for modelling contributing factors, feedback loops, evidence, uncertainty, and interventions in complex systems. It is designed for teams asking questions such as:

- Which conditions are driving the outcome we care about?
- Where do reinforcing or balancing loops exist?
- How uncertain are our cost, duration, effect, and success assumptions?
- Which assumptions are shared or correlated?
- What changed in the model, and can another client replay it deterministically?

## Start with a real problem

A delivery team wants to understand why lead time remains high. It models:

1. **Reliable delivery** as an outcome.
2. **Fast feedback**, **small batches**, and **learning rate** as factors.
3. **Deployment automation** as an intervention.
4. Current states, desired states, costs, durations, and causal effects as typed uncertain estimates.
5. A scenario containing objectives, budget, planning horizon, and candidate interventions.

Optimist can already validate and store that model, detect structural feedback loops, update selected priors with conjugate Bayesian evidence, and evaluate unit-checked Fermi decompositions. Finite-horizon decision propagation and the visual workbench are the next major implementation stage.

## Try the core

```sh
cargo run --example feedback_loop
cargo run --example fermi_delivery_time
cargo run --example bayesian_delivery_success
```

Or start the server and use the CLI:

```sh
cargo run -- server
cargo run -- project create "Delivery reliability"
cargo run -- --project A node create \
  --kind outcome \
  --name reliable_delivery \
  --title "Reliable delivery" \
  --direction maximize
```

Continue with the [getting-started guide](./guide/README.md).

::: warning Development status
The default server keeps projects in memory. Restarting it loses project data. The modelling, probability, structural-analysis, command-replay, and WebSocket cores are implemented and tested; durable project storage, full Markdown import/export transport, decision propagation, and the Vue workbench remain under development.
:::
