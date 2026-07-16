# Examples

The repository includes small, executable Rust programs which use Optimist's public API. They are intended as starting points for your own model construction and analysis.

## Feedback loop discovery

```sh
cargo run --example feedback_loop
```

Builds three factors and causal edges, computes exact strongly connected components, and enumerates the loop in canonical order.

[Read the source](https://github.com/SierraSoftworks/optimist/blob/main/examples/feedback_loop.rs)

## Fermi delivery-time estimate

```sh
cargo run --example fermi_delivery_time
```

Combines uncertain deployment count and minutes per deployment using a unit-checked multiplication formula. Deterministic Monte Carlo sampling reports the resulting monthly effort and separates model variance from simulation error.

[Read the source](https://github.com/SierraSoftworks/optimist/blob/main/examples/fermi_delivery_time.rs)

## Bayesian rollout success

```sh
cargo run --example bayesian_delivery_success
```

Starts from a Beta prior, observes 17 successes in 20 trials, and applies the exact Beta-Binomial conjugate update. The assertions verify that evidence moves the posterior and contracts its uncertainty.

[Read the source](https://github.com/SierraSoftworks/optimist/blob/main/examples/bayesian_delivery_success.rs)

## Verify every example

```sh
cargo check --examples
cargo run --example feedback_loop
cargo run --example fermi_delivery_time
cargo run --example bayesian_delivery_success
```

For a server-backed model, continue with [Getting started](../guide/README.md).
