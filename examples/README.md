# Worked examples

Each directory here is a design the tool can load, solve, and rank. They are
meant to be read as much as run, so the reasoning behind every quantity lives in
the `summary` fields beside it rather than in this file.

Both are covered by tests that assert the conclusions they claim to teach, so an
engine change that quietly stops one from demonstrating its point fails the
build rather than the reader.

## checkout

A shop front: browsers reach an API pool that reads from an order store, with
retries in front of the pool and a cache in front of the store.

```sh
cargo run -- check       examples/checkout
cargo run -- solve       examples/checkout
cargo run -- bottlenecks examples/checkout
cargo run -- compare     examples/checkout warm-cache
```

It is the introduction: a design small enough to hold in your head, where the
binding constraint turns out to be the one nobody watches. Thirty days of
retention overruns the store several times over, while the pool everyone worries
about binds in a third of draws despite a mean that looks survivable. Neither
proposed change fixes it — caching relieves the store and leaves the pool exactly
where it was; a bigger pool relieves the pool and pushes more traffic at a store
that already could not cope.

## metastable

A checkout service whose dependency connections are held for the whole life of a
request, which is enough to give the design two steady states at one level of
demand.

```sh
cargo run -- solve   examples/metastable --horizon 25
cargo run -- compare examples/metastable no-surge --horizon 25
cargo run -- compare examples/metastable shed     --horizon 25
```

A ten-second surge ends ten seconds before the model is read, and demand is back
to a level the design served comfortably before. It does not recover. The
`no-surge` comparison is the same design under the same demand at the same
moment, differing only in what happened earlier, and it sits at a quarter of the
occupancy.

Three things in it are worth the read:

- **Two thresholds, far apart.** The healthy state stops existing above
  `C/(4ds)`, and the collapsed state stops existing below `(C/D)(1 - ds/D)`.
  Demand landing between them can be in either, and which one depends on history.
  Both are computed in the scratchpad rather than asserted, so changing a
  parameter moves them.
- **The deadline is the constraint, not the pool.** Collapsed, the pool runs at
  91% — congested but not exhausted. What is exhausted is the request budget, and
  a design read only through its connection limit would report headroom.
- **Lengthening the deadline makes it worse.** The reflex when requests fail. It
  lowers the release load, because a request that is going to fail anyway holds
  its connection for longer before being told so.

Both component types this design uses are defined in its own `component-types/`
directory. The engine was not changed to support it, which is the property that
makes the catalogue worth having.
