# Analysis

Optimist provides exact structural graph analysis and finite-horizon Monte Carlo projection for scenario candidates. The two results are intentionally separate: topology is exact, while intervention impact depends on explicit modelling assumptions and uncertain inputs.

## Causal graph projection

Only these edge kinds participate in structural causal analysis:

- `contributes`,
- `changes`,
- `blocks`.

`requires`, `part-of`, `measures`, `conflicts-with`, and `synergizes-with` do not enter causal strongly connected component calculation. `requires`, conflicts, and synergies separately inform intervention execution readiness and scenario projection.

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

Analyze every candidate execution plan in a stored scenario:

```sh
cargo run -- --project A scenario analyze A
```

Every factor and objective on a candidate-to-objective causal path needs a native quantity and `current` estimate. For `A requires B`, B executes before A. Recursive prerequisite durations add, each required intervention must succeed, and successful prerequisite `changes` effects enter propagation before the candidate. Hard factor requirements preclude execution when their threshold is absent or unsatisfied. `changes` and `contributes` provide sampled dimensionless local responses.

A response is a ratio, so a state moves relative to its own sampled baseline rather than by an amount expressed in its unit. How responses combine follows the destination quantity's declared support, because support is what makes one rule sound. A strictly non-negative quantity has a ratio scale, so its responses multiply:

$$
x_i(t) = \operatorname{clamp}_i\left(
  b_i \prod_j \left(\frac{x_j(t-d_{ji})}{b_j}\right)^{\varepsilon_{ji}}
\right).
$$

A plain product such as $\text{impact} = \text{frequency} \times \text{duration}$ is therefore two edges with $\varepsilon = 1$, and the result stays non-negative for free. A quantity that may be zero or negative has no ratio scale, so its responses accumulate against baseline instead:

$$
x_i(t) = \operatorname{clamp}_i\left(
  b_i \left(1 + \sum_j \varepsilon_{ji}\left(\frac{x_j(t-d_{ji})}{b_j} - 1\right)\right)
\right).
$$

A response with no explicit lag consumes its source from the previous planning period. Explicit duration and lag estimates are interpreted as numbers of planning periods, rounded up, and added to that one-period transport delay. An unchanged source has a ratio of one and so contributes no movement, and $\operatorname{clamp}_i$ applies the destination quantity's declared support. A source whose sampled baseline is zero has no fractional movement at all; its responses are dropped and reported as undefined rather than propagating an infinity. Horizons are bounded to 10,000 periods.

### Time-boxed interventions

An intervention has no level to take a ratio of, so a `changes` response is the multiplier $m_k$ applied to its target while the intervention is fully active. Its temporal activation $a_k(t)$ enters as the exponent, contributing $m_k^{a_k(t)}$ multiplicatively or the share $(m_k - 1)a_k(t)$ additively, with the sampled rebound multiplier $\rho_k$ applied the same way against $b_k(t)$. Ramping therefore interpolates geometrically. An effect without a profile holds $a_k = 1$ and $b_k = 0$ after arrival, which is the monotone step a permanent intervention applies, so adding a profile changes only an effect's schedule and never its magnitude.

A profile has four parts. `ramp` spends $r$ periods rising to full strength, `hold` keeps $h$ periods at full strength, `release` returns the effect toward zero, and an optional `aftereffect` fires a rebound when the release begins:

$$
a_k(e) = \begin{cases}
  \dfrac{e+1}{r+1} & e < r \\[6pt]
  1 & r \leq e < r + h \\[4pt]
  \sigma(e - r - h) & e \geq r + h
\end{cases}
$$

where $e$ counts periods since arrival and $\sigma$ is the release kernel: $0$ for an abrupt end, $\max(0,\,1-\frac{k+1}{L+1})$ for a decline over $L$ periods, and $2^{-(k+1)/H}$ for a half-life $H$. Omitting `hold` leaves the effect permanent and removes the release phase entirely.

The rebound carries its own magnitude rather than a share of the primary effect, because ending an intervention is its own event: a backlog that drains after a change freeze rarely returns exactly what was withheld. Every profile duration is an ordinary Squiggle estimate, so a schedule is as uncertain as any other input, and durations are sampled per draw and rounded up to whole periods. Only `changes` effects accept a profile; a `contributes` relationship is always in effect and has no activation to start or stop.

Modelling a time-boxed intervention this way removes the older workaround of adding a placeholder factor whose only job was to fire a lagged rebound.

Each candidate run uses the scenario's pinned ChaCha20 seed and Monte Carlo stopping controls. Baselines, prerequisite and candidate success, cumulative duration, lags, and destination responses are sampled once per joint draw. Reports include total execution duration, all-steps success, prerequisite/blocker/synergy/conflict context, baseline, final-state, and direction-oriented improvement means and variances, covariance between objective improvements, reachability, clamping, Monte Carlo errors, and convergence status. Improvement is always a relative, preference-oriented delta from baseline: positive means improvement even for a minimize objective.

Repeating the same immutable revision and seed is bit-reproducible for the current algorithm and pinned dependency versions. Adding or reordering sampled model inputs changes the random stream and therefore the exact sample sequence.

::: warning Current statistical boundary
Primitive estimates are sampled independently. A non-empty project dependence model causes scenario analysis to fail explicitly until correlated dynamic sampling is implemented. Candidate execution plans are still evaluated one at a time; budgets, costs, numeric synergy magnitudes, candidate bundles, and scalar utility are not yet optimization inputs. Synergy and conflict edges are reported as qualitative decision context.
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
