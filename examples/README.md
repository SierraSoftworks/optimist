# Worked examples

Each directory here is a design the tool can load, solve, and rank. They are
meant to be read as much as run, so the reasoning behind every quantity lives in
the `summary` fields beside it rather than in this file.

Both are covered by tests that assert the conclusions they claim to teach, so an
engine change that quietly stops one from demonstrating its point fails the build
rather than the reader.

## checkout

A shop front: browsers reach an API pool that reads from an order store, with
retries and a timeout in front of the pool and a cache in front of the store.

```sh
cargo run -- check       examples/checkout
cargo run -- solve       examples/checkout
cargo run -- bottlenecks examples/checkout
cargo run -- compare     examples/checkout warm-cache
cargo run -- compare     examples/checkout bigger-pool
```

It is the introduction: a design small enough to hold in your head, where the
binding constraint turns out to be the one nobody watches. Thirty days of
retention overruns the store several times over, and the success objective the
client declares is missed in most draws, while the pool everybody worries about
is third on the list.

Neither proposed change fixes it. Caching relieves the store outright and loads
the pool slightly, because requests that used to fail now reach it; a bigger pool
relieves the pool and pushes more traffic at a store that already could not cope.

## metastable

A checkout service whose workers are held for the whole of a downstream call,
behind a retry policy. The design has two steady states over a band of load, and
the point of the model is that neither threshold is where intuition puts it: the
load that tips the system over is not the load it recovers at.

```sh
cargo run -- solve       examples/metastable --horizon 25
cargo run -- bottlenecks examples/metastable --horizon 25
cargo run -- compare     examples/metastable no-surge       --horizon 25
cargo run -- compare     examples/metastable shed           --horizon 25
cargo run -- compare     examples/metastable fewer-retries  --horizon 25
cargo run -- compare     examples/metastable longer-timeout --horizon 25
```

A ten-second surge ends ten seconds before the model is read, and demand is back
to a level the design served comfortably before. It does not recover. The
`no-surge` comparison is the same design under the same demand at the same
moment, differing only in what happened earlier, and it is not in trouble at all.

Four things in it are worth the read:

- **Nothing here is bespoke.** Every part is drawn from the shipped catalogue.
  The trap is not written into any one component; it emerges from three ordinary
  decisions meeting — workers wait on a dependency, a timeout calls slowness a
  failure, and a retry policy answers failure with more load.
- **Two thresholds, far apart.** The load at which the healthy state stops
  existing and the load at which the collapsed state stops existing are different
  numbers. Demand landing between them can be in either state, and which one
  depends on history. Both are computed in the scratchpad rather than asserted,
  so changing a parameter moves them.
- **The pool is not the constraint.** What is exhausted is the request budget and
  the connections held against the dependency. A design read only through its
  worker count would report headroom.
- **Lengthening the deadline makes it worse.** The reflex when requests fail. It
  lowers the load at which the design releases, because a request that is going
  to fail anyway holds its connection for longer before being told so.
