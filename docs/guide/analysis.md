# Structural analysis

Optimist currently provides exact structural analysis of the causal graph. This is intentionally separate from statistical intervention propagation.

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

## What structural analysis does not claim

Current structural output does **not** establish:

- whether a loop is stable,
- whether it is reinforcing or balancing with a particular probability,
- equilibrium values,
- intervention-to-outcome impact,
- causal identification from observational data,
- a ranked investment frontier.

Those require signed uncertain effects, lags, intervention success, scenario horizons, dependence, and explicit propagation assumptions. Optimist keeps these roadmap items separate to avoid presenting topology as statistical evidence.

## Interpreting results

Treat SCCs and cycles as candidates for review:

1. Read each edge mechanism and evidence.
2. Confirm edge direction and endpoint semantics.
3. Check whether the cycle is meaningful at one time scale.
4. Review lags and uncertain effect signs.
5. Decide whether omitted common causes require dependence modelling.
6. Only then apply a documented stability or propagation model.
