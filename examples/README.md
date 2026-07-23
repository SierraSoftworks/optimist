# Optimist examples

These examples exercise the public Rust API and are compiled by `cargo check --examples`.

## Feedback loop discovery

```sh
cargo run --example feedback_loop
```

Builds a three-factor reinforcing loop with typed causal effects, then computes exact strongly connected components and bounded elementary cycles. Use it as a starting point for structural feedback analysis.

## Bayesian rollout success

```sh
cargo run --example bayesian_delivery_success
```

Updates a Beta prior after observing 17 successful rollouts in 20 trials. It demonstrates the validated Beta-Binomial likelihood and verifies that the posterior moves towards the evidence while uncertainty contracts.

## Running all examples

```sh
cargo check --examples
cargo run --example feedback_loop
cargo run --example bayesian_delivery_success
```

For the server-backed workflow, follow the [getting-started guide](../docs/guide/README.md).
