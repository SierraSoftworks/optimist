---
mode: agent
description: Cut what a design that will not settle costs to solve, and say so sooner.
---

# Settle faster, or give up sooner

## The problem

A design with no single steady state costs far more to solve than one that has
it, and nearly all of that cost buys nothing. `optimist bottlenecks
./examples/saturation/ --horizon 20` originally took over two minutes because
every step past the sixth ran to the iteration cap. It now takes about 3.4
seconds, but the gap is still there:

| scenario | passes per step |
| --- | --- |
| `checkout`, transient, 60 steps | ~56 |
| `saturation`, transient, 20 steps | ~195 |

Both are reported as settled. The saturation design *does* come to rest — on a
mixture of states rather than a single value per draw — so this is no longer a
failure to converge. It is how long recognising that takes.

Your goal is to close that gap: a design that rests on a mixture should cost
about what a design that rests on one state costs.

## Where the code is

- `src/system/evaluate/relax.rs` — the relaxation loop, the adaptive damping, and
  the stall check. `PATIENCE = 128` is the floor: a step that is not going to
  settle must run at least that many passes before the check fires. `PROGRESS =
  0.98` is the improvement it looks for over that span.
- `src/system/evaluate/stationary.rs` — decides that an iterate has come to rest
  on several states rather than one.
- `src/system/evaluate/state.rs` — `Step::mixture` and `Step::unsettled` are what
  a step reports about not settling. `Unsettled::stalled` distinguishes an
  iterate that stopped improving from one that merely ran out of passes.
- `src/system/evaluate/config.rs` — `max_iterations` (1,500), `tolerance` (1e-6),
  `damping` (0.2).

## Where to look first

The mixture is detected, so the question is why it takes ~195 passes to say so.
Measure before changing anything: how many passes elapse before `stationary`
could first have concluded, against how many it actually takes. If the answer is
that the check only runs on the `PATIENCE` boundary, the fix is to run it sooner
or more often rather than to lower `PATIENCE`.

Lowering `PATIENCE` is the tempting move and is probably wrong on its own. A
design whose loop gain sits just under one converges genuinely but without hurry,
and cutting it off reports a settled design as unsettled. The damping comment in
`relax.rs` and the `max_iterations` comment in `config.rs` both explain why the
numbers are what they are — read them before changing them.

## Constraints

- **Do not report a converging design as unsettled.** This is the failure mode
  that matters; it is worse than being slow.
- **The golden baselines must not move.** `tests/golden/*.json` records what each
  shipped example settles on. If your change moves them, it has changed an
  answer, not just how fast it arrived.
- The behavioural tests are the real guard: `tests/system_saturation.rs`,
  `tests/system_metastable.rs`, `tests/system_queued_collapse.rs`,
  `tests/system_deadlines.rs` and `tests/system_example.rs` assert what each
  design demonstrates rather than its exact numbers.
- Bistable designs are sensitive to the path taken. Changing damping or pass
  counts can land a draw on the other branch — see
  `tests/system_divided.rs::BISTABLE`.

## Verifying

```sh
cargo nextest run --release
cargo nextest run --release --features profiling -E 'binary(system_profile)' --no-capture
cargo bench --bench solve
```

The profile harness prints passes per scenario, including the non-settling one.
Report passes per step before and after, for both a settling and a non-settling
design, and confirm the golden baselines are untouched.

## Worth knowing

`Unsettled` already names the component and channel that was still moving. It is
not yet surfaced by the CLI or the workbench, and doing so is a small, separate,
genuinely useful piece of work: "utilisation on `api` is still moving by a tenth
every pass" sends an author to the loop that will not close, where "did not
settle" sends them through the whole design.
