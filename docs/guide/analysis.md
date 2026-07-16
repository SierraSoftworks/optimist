# Analysis

Optimist provides exact structural graph analysis and finite-horizon Monte Carlo projection for scenario candidates. The two results are intentionally separate: topology is exact, while intervention impact depends on explicit modelling assumptions and uncertain inputs.

## Causal graph projection

Only these edge kinds participate in structural causal analysis:

- `contributes`,
- `changes`,
- `blocks`.

`requires`, `part-of`, `measures`, `conflicts-with`, and `synergizes-with` retain important model semantics but do not enter causal strongly connected component calculation.

An immutable analysis key records:

- project ID,
- independent graph revision,
- optional scenario ID and revision,
- dependence document revision,
- formula document revision.

This makes it possible to detect whether a result was computed from the documents a caller expected.

## Strongly connected components

Optimist uses Tarjan's algorithm to partition every node exactly once into maximal strongly connected components (SCCs).

A component represents feedback when it contains:

- more than one node, or
- one node with a causal self-loop.

SCC detection is exact and deterministic. Component members and internal edges are sorted by canonical IDs.

## Elementary cycles

Within the causal graph, Optimist enumerates directed elementary cycles without repeating internal nodes. Enumeration is bounded by:

- maximum edges per cycle,
- maximum returned cycle count.

```sh
cargo run -- --project A analysis structure \
  --maximum-cycle-length 8 \
  --maximum-cycles 1000
```

If another cycle exists beyond the count bound, `cycles_truncated` is set. A returned cycle is rotated so its smallest entity ID appears first, giving a deterministic representation.

The [feedback-loop example](../examples/#feedback-loop-discovery) builds and verifies a three-factor loop:

```sh
cargo run --example feedback_loop
```

## Scenario-scoped keys

You may include a scenario revision in the projection key:

```sh
cargo run -- --project A analysis structure --scenario A
```

The graph topology is unchanged by selecting a scenario. The scenario revision is retained so later decision-analysis results can prove which objectives, budget, horizon, and candidates were selected.

## Finite-horizon scenario projection

Analyze every candidate in a stored scenario independently:

```sh
cargo run -- --project A scenario analyze A
```

Every factor and objective on a candidate-to-objective causal path needs a `current` normalized-state estimate. Candidate interventions use their optional completion duration and probability of success. `changes`, `contributes`, and factor-to-state `blocks` effects provide sampled signed local effects.

For sampled baseline $b_i$, state $x_i(t)$, persistent intervention shift $u_i(t)$, edge effect $e_{ji}$, and delay $d_{ji}$, Optimist applies the synchronous recurrence

$$
x_i(t) = \operatorname{clamp}\left(
  b_i + u_i(t) + \sum_j e_{ji}\left(x_j(t-d_{ji})-b_j\right),
  0,
  1
\right).
$$

An effect with no explicit lag consumes its source from the previous planning period. Explicit duration and lag estimates are interpreted as numbers of planning periods, rounded up, and added to that one-period transport delay. Intervention changes persist after their sampled completion and lag. The recurrence uses deviations from sampled baselines so an unchanged source contributes zero movement. Horizons are bounded to 10,000 periods.

Each candidate run uses the scenario's pinned ChaCha20 seed and Monte Carlo stopping controls. Baselines, success, duration, lags, and edge effects are sampled once per joint draw. Reports include baseline, final-state, and direction-oriented improvement means and variances, covariance between objective improvements, candidate-to-objective reachability, clamped state-update counts, Monte Carlo standard errors, valid/attempted counts, invalid-draw counts, and convergence status. An unreachable objective retains its sampled baseline and zero movement, explicitly marked `reachable: false`. A nonzero clamp count indicates saturation or potentially unstable feedback and deserves model review.

Repeating the same immutable revision and seed is bit-reproducible for the current algorithm and pinned dependency versions. Adding or reordering sampled model inputs changes the random stream and therefore the exact sample sequence.

::: warning Current statistical boundary
Primitive estimates are sampled independently. A non-empty project dependence model causes scenario analysis to fail explicitly until correlated dynamic sampling is implemented. Candidates are evaluated one at a time; budgets, costs, prerequisites, conflicts, synergies, candidate bundles, and scalar utility are not yet optimization inputs.
:::

## What analysis does not claim

Current structural and scenario output does **not** establish:

- whether a loop is stable,
- whether it is reinforcing or balancing with a particular probability,
- equilibrium values,
- causal identification from observational data,
- a ranked investment frontier.

Finite-horizon projection estimates impact under the recurrence and supplied priors; it does not prove that an edge is causal or identify effects from observational data. Stable feedback, dependence-aware dynamics, bundles, costs, and Pareto ranking remain separate roadmap items.

## Interpreting results

Treat SCCs and cycles as candidates for review:

1. Read each edge mechanism and evidence.
2. Confirm edge direction and endpoint semantics.
3. Check whether the cycle is meaningful at one time scale.
4. Review lags and uncertain effect signs.
5. Decide whether omitted common causes require dependence modelling.
6. Only then apply a documented stability or propagation model.
