# Worked examples

Each directory here is a design the tool can load, solve, and rank. They are
meant to be read as much as run, so the reasoning behind every quantity lives in
the `summary` fields beside it rather than in this file.

All five are covered by tests that assert the conclusions they claim to teach,
so an engine change that quietly stops one from demonstrating its point fails
the build rather than the reader.

The first three build on each other and are best read in order.

## saturation

Browsers call an API, the API reads from a store, and that read is wrapped in
the two policies every client library ships with — a timeout and a retry.

```sh
cargo run -- solve   examples/saturation --horizon 10
cargo run -- compare examples/saturation no-retries      --horizon 10
cargo run -- compare examples/saturation patient-timeout --horizon 10
```

**Saturation has a closed form.** Serving λ operations each held for L seconds
puts λL in flight, and a pool at fraction ρ of its limit answers in S/(1−ρ).
Together those give L = S / (1 − λL/C), which has real solutions only while
4λS ≤ C. The design folds at C/4S — a number built from an idle latency and a
pool size, neither of which is a throughput figure and neither of which appears
on a capacity plan. Measured: the store answers in 11 ms at rest and 3.5 s at
3.5× demand, with its own service time unchanged.

**Retrying past the fold lowers success rather than protecting it**, because the
failures it answers are congestion it caused. At the surge, three attempts give
71% success where one gives 85%. The deadline decides how much this costs: at a
30 ms budget the same comparison is 43% against 84%, and beyond about six times
the dependency's idle latency the timeout never fires before the fold and the
retry policy is inert.

**It recovers the moment demand does.** A socket buffer holds a few hundred
requests and drains against a surplus of thousands per second, so a fivefold
change in wire depth makes no difference to recovery at all. That is the
property the next example does not have.

## queued-collapse

The same shop front, plus a queue and a fulfilment worker drawing on the same
store.

```sh
cargo run -- solve   examples/queued-collapse --transient --horizon 280 --step 0.5
cargo run -- compare examples/queued-collapse no-surge --transient --horizon 280 --step 0.5
cargo run -- compare examples/queued-collapse shed     --transient --horizon 280 --step 0.5
```

One addition makes the design second order. A queue holds tens of thousands of
operations and what it holds it keeps, so the backlog is a state variable — and
draining it is itself load on the store the user-facing path depends on.

**Recovery outlasts the cause by an order of magnitude.** A ten second surge
builds a backlog of 6,700 jobs, 13 seconds of staleness, which is still being
worked off seventy seconds later.

**The design has two steady states at one level of demand.** Both readings below
are at t=140s, at the same offered load, with the queue empty in each:

| | user latency | store latency | backlog |
| --- | --- | --- | --- |
| `no-surge` | 0.0132 s | 0.0111 s | 0 |
| after the surge | 0.9008 s | 0.2530 s | 0 |

They differ only in what happened two minutes earlier. Nothing in the model
asserts this; it emerges from draining being load.

**Shedding at the edge is the only lever that works**, and its limit has to be
set from what the consumer drains at rather than from what the front end could
serve — which is why a limit chosen from the front end's headroom is always too
high. It is charged for honestly: half the traffic is refused while the surge
lasts, and the client's objective reports it.

## deadlines

A search path three services deep, where one request becomes six operations in
the service at the bottom. Every hop has a timeout. Only one thing varies:
whether giving up is passed on.

```sh
cargo run -- solve   examples/deadlines --transient --horizon 25 --step 0.5
cargo run -- compare examples/deadlines leaf-timeouts    --transient --horizon 25 --step 0.5
cargo run -- compare examples/deadlines single-operation --transient --horizon 25 --step 0.5
```

A timeout does two things that are usually confused for one. It bounds what the
caller waits for, and — only if the cancellation travels — it withdraws the work
nobody is waiting for any more. The first protects the caller; the second is the
only one that protects the dependency, and it is the one a timeout implemented
as a local `select` on a socket does not do.

**Failing to propagate is invisible from where most teams measure.** Success is
0.933 either way. What changes is that the search service is twice as occupied,
holding requests whose answers have already been thrown away.

**The penalty scales with follow-on work.** With one operation per request
nothing congests at all; with six, the index is 58× slower. Work abandoned at
the top has already become six times as much work at the bottom, and none of it
is recalled unless the giving up reached there.

This design defines its own behaviour in `mutators/`, because an intervention
rebinds quantities rather than changing structure — which is what makes the two
readings comparable rather than two designs differing in untracked ways.

## checkout

A shop front where the binding constraint is the one nobody watches.

```sh
cargo run -- bottlenecks examples/checkout
cargo run -- compare     examples/checkout warm-cache
```

Thirty days of retention overruns the store several times over, and the success
objective the client declares is missed in most draws, while the pool everybody
worries about is third on the list. Neither proposed change fixes it: caching
relieves the store and loads the pool, and a bigger pool pushes more traffic at
a store that already could not cope.

## metastable

A checkout service whose workers are held for the whole of a downstream call,
behind a retry policy, with two steady states over a band of load.

```sh
cargo run -- compare examples/metastable no-surge       --horizon 25
cargo run -- compare examples/metastable longer-timeout --horizon 25
```

Every part is drawn from the shipped catalogue; the trap emerges from three
ordinary decisions meeting — workers wait on a dependency, a timeout calls
slowness a failure, and a retry answers failure with more load. The load that
tips the system over is not the load it recovers at, and lengthening the
deadline, the reflex when requests fail, makes it worse.
