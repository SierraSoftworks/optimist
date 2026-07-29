---
mode: agent
description: Iterate draws on the outside so a solve stops building arrays, and can be split across cores.
---

# Put the draw index on the outside

## The problem

The solver evaluates a whole model over a thousand draws at once: every value is
an array, and every operation builds another one. One steady solve of
`examples/checkout` still materialises about 100,000 draws per pass across ~365
passes, and roughly 15,200 of those arrays come from the elementwise driver
alone.

The alternative is to turn the loops inside out. Each draw index carries an
independent deterministic system — this is stated in the module documentation of
`src/system/evaluate/mod.rs` and is the property the whole design rests on — so
the model could be solved one draw at a time, with the ensemble loop outermost.
Then no value holds more than one `f64`.

That change is worth making for three reasons, of which speed is only the first:

1. **Allocation disappears.** A distribution needs a one-slot memo, not an array,
   because the draw index is fixed for the whole of one draw's relaxation.
2. **Convergence becomes per draw.** Today `movement` is a max over every draw, so
   one draw sitting near a fold holds the whole model at a twentieth of the step
   — the damping comment in `relax.rs` describes this. Per draw, the ones that
   have settled stop being worked on.
3. **Parallelism becomes trivial and exact**, rather than the coarse-grained
   partitioning that exists now.

## What is already in place

Do not rebuild these.

- `src/squiggle/distribution/indexed.rs` — a draw computed from its index alone:
  a four-round Feistel permutation over the padded domain with cycle walking,
  measured at 14 ns per draw. Bijective by construction, stratified, and
  decorrelated across sites. This is the piece that makes the transposition
  possible at all.
- `src/squiggle/distribution/ensemble.rs` — `Ensemble` separates the ensemble
  *size* (which every thread samples identically) from the *share* of it a worker
  computes. The share is a fraction, not an index range, so it lands correctly on
  authored sample sets shorter than the configured count.
- `src/squiggle/distribution/draws.rs` — a value carries a seed in a shared
  `Stream` handle rather than a sample set. Clones share the handle, which is what
  keeps `x - x` exactly zero while two textually identical constructors stay
  independent.
- `src/squiggle/snapshot.rs` — `Transferred` is `Value` without its function
  variant, so the compiler derives `Send`. A `Value` cannot cross a thread
  boundary in either direction; a worker snapshots its own result on the way out.
- `src/system/evaluate/merge.rs`, `EvaluationConfig::threads` and `divided()` —
  a solve can already be split by draws and reassembled, sequentially.

## Things that were measured and did not work

Each of these looks obviously right and is not. Do not spend the day rediscovering
them.

| attempt | result on `steady/checkout` |
| --- | --- |
| baseline | 325 ms |
| reaching through the distribution per draw index instead of resolving its share to an array once | 463 ms |
| dropping the resolved-share cache entirely, so primitives are re-inverted per read | 591 ms |
| `rayon` across draws inside `elementwise` | ~2,070 ms |
| `Arc<Vec<f64>>` instead of `Arc<[f64]>` to avoid a copy | 331 ms |

The lesson common to the first three: **while the draw loop is on the inside, the
inner loop must index memory.** A one-slot memo only beats a resolved array once
the index is fixed for a whole relaxation, which is exactly what this change makes
true — so the numbers above are not arguments against the transposition, they are
the reason it has to be done properly rather than halfway.

`rayon` lost because the unit of work was one formula over 1,000 draws. With the
index outside, the unit of work becomes a whole relaxation, which is the right
granularity.

## The hard parts

- **Aggregate statistics need the whole set.** `mean`, `quantile`, `stdev` and
  `mode` over a sample set are not per-draw quantities. `src/squiggle/distribution/stats.rs`
  is analytic for symbolic families, which helps, but empirical sets still need
  materialising on demand.
- **Expression trees must not grow across passes.** If composition stays symbolic
  and a channel's value references the previous pass's value, the tree gains a
  level per pass and never stops. Whatever the design, something must collapse it
  at the pass boundary.
- **A value referenced twice must not be recomputed twice.** Diamonds in the
  expression graph are the trap; memoise by node identity within one draw.
- **Bistable designs follow the path taken.** Changing what damps against what can
  land a draw on a different branch. `tests/system_divided.rs` separates `SETTLING`
  from `BISTABLE` designs for exactly this reason.
- **Determinism is a guarantee.** Identical source, modules and configuration must
  replay exactly. Seeds come from the run's generator on first use; pointer
  addresses are not reproducible.

## Verifying

```sh
cargo nextest run --release
cargo bench --bench solve
cargo nextest run --release --features profiling -E 'binary(system_profile)' --no-capture
```

Expect the golden baselines to move: reordering when draws are computed changes
floating-point accumulation. That is acceptable **only** if the behavioural tests
pass untouched — `tests/system_example.rs`, `system_saturation.rs`,
`system_metastable.rs`, `system_deadlines.rs`, `system_queued_collapse.rs` assert
what each design demonstrates rather than its numbers. Re-record with
`env UPDATE_GOLDEN=1 cargo nextest run --release -E 'binary(system_golden)'` and
say in the commit message why they moved.

`tests/system_divided.rs` must keep passing: a solve divided across draws has to
agree with an undivided one, to the convergence tolerance.

## Sequencing

The change is large enough that it should land in steps that each keep the tree
green. A reasonable order:

1. A per-draw evaluation path alongside the existing one, exercised by tests only.
2. The solver state expressed per draw, with the ensemble loop still outermost in
   name only.
3. Convergence per draw, which is where the pass-count win appears.
4. Parallelism across draw blocks, replacing `EvaluationConfig::divided()`.

Measure after each. The benchmark harness and the counters exist so that no step
lands on reasoning alone.
