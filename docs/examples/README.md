# Examples

Two designs ship with the repository, in `examples/`. They are meant to be read
as much as run: the reasoning behind every quantity lives in the `summary` fields
beside it rather than in prose elsewhere.

Both are covered by tests that assert the conclusions they claim to teach, so an
engine change that quietly stops one from demonstrating its point fails the build
rather than the reader.

## `checkout`

A shop front. Browsers reach an API pool that reads from an order store, with
retries and a timeout in front of the pool and a cache in front of the store.

```sh
cargo run -- check       examples/checkout
cargo run -- solve       examples/checkout
cargo run -- bottlenecks examples/checkout
cargo run -- compare     examples/checkout warm-cache
cargo run -- compare     examples/checkout bigger-pool
```

It is the introduction: a design small enough to hold in your head, where the
binding constraint turns out to be the one nobody watches.

```text
COMPONENT  CONSTRAINT          UTILISATION  P90      BINDS  REPLICAS  HEADROOM
orders     volume              7.009        9.555    100%   1         -3004303674979.1333
api        capacity            2.960        4.916    87%    1         -1063.2349
browsers   success_objective   55.626       109.865  86%    1         -0.2731
browsers   latency_objective   0.460        0.793    3%     1         0.4053
```

Thirty days of retention overruns the store several times over, and the design
misses the success objective its client declares in most draws. The pool
everybody worries about is third.

Neither proposed change fixes it. `warm-cache` relieves the store outright and
loads the pool slightly, because requests that used to fail now reach it.
`bigger-pool` relieves the pool and pushes more traffic at a store that already
could not cope. This is the ordinary shape of a capacity decision: relieving one
limit promotes another, and `compare` says so.

**Read it for:** the three shipped types that appear in nearly every design
(`client`, `compute`, `datastore`), how behaviours attach to a relationship, how
the scratchpad and interventions work together, and why the ranking is by
probability of binding rather than by mean utilisation.

## `metastable`

A checkout service whose workers are held for the whole of a downstream call,
behind a retry policy. The design has two steady states over a band of load, and
the point of the model is that neither threshold is where intuition puts it: the
load that tips the system over is not the load it recovers at.

```sh
cargo run -- solve       examples/metastable --horizon 25
cargo run -- bottlenecks examples/metastable --horizon 25
cargo run -- compare     examples/metastable no-surge        --horizon 25
cargo run -- compare     examples/metastable shed            --horizon 25
cargo run -- compare     examples/metastable fewer-retries   --horizon 25
cargo run -- compare     examples/metastable longer-timeout  --horizon 25
```

A ten-second surge ends ten seconds before the model is read, and demand is back
to a level the design served comfortably before. It does not recover.

```sh
cargo run -- compare examples/metastable no-surge --horizon 25
```

```text
COMPONENT  CONSTRAINT          BEFORE   AFTER  BOUND BEFORE  BOUND AFTER  EFFECT
checkout   capacity            179.907  0.078  100%          0%           relieved
shoppers   success_objective   99.993   0.002  100%          0%           relieved
inventory  concurrency         2.272    0.165  100%          0%           relieved
shoppers   latency_objective   1.500    0.134  100%          0%           relieved
```

`no-surge` is the same design under the same demand at the same moment. It
differs only in what happened earlier, and it is not in trouble at all.

Four things in it are worth the read.

**Nothing here is bespoke.** Every part is drawn from the shipped catalogue. The
trap is not written into any one component: it emerges from three ordinary
decisions meeting — workers wait on a dependency, a timeout calls slowness a
failure, and a retry policy answers failure with more load.

**Two thresholds, far apart.** The load at which the healthy state stops existing
and the load at which the collapsed state stops existing are not the same number.
Demand landing between them can be in either state, and which one depends on
history. Both are computed in the scratchpad rather than asserted, so changing a
parameter moves them.

**The pool is not the constraint.** What is exhausted is the request budget and
the connections held against the dependency, and a design read only through its
worker count would report headroom.

**Lengthening the deadline makes it worse.** `longer-timeout` is the reflex when
requests fail, and it lowers the load at which the design releases, because a
request that is going to fail anyway holds its connection for longer before being
told so.

**Read it for:** time-dependent demand in the scratchpad, why `--horizon` matters
even under steady solving, how the same intervention machinery expresses "what if
this had never happened", and what a design near a fold looks like in the spread
of its results.

## Using them as a starting point

Copy a directory and edit it. There is nothing to register and no database to
migrate:

```sh
cp -r examples/checkout designs/my-service
cargo run -- check designs/my-service
```

Or serve the whole directory and open it in the workbench:

```sh
cargo run -- serve --designs ./designs
```

Continue with [designing a system](../guide/modelling.md).
