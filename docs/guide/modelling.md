# Designing a system

A design is a graph of typed components wired together. This page covers the
seven things a design is made of and how they fit together; the file layout they
are stored in is in the [YAML reference](../reference/yaml.md).

## Components

A component is one part of the system. It adopts a **component type**, which
decides what properties it must supply, what quantities it derives, where
relationships may attach, and what resource limits it can saturate.

```yaml
id: api
name: Checkout API
type: compute
properties:
  service_time: lognormal(-4.6, 0.35)
  parallelism: pool_size
  replicas: '1'
```

Every property is Squiggle source, not a number. `lognormal(-4.6, 0.35)` is a
distribution; `pool_size` is a reference to a shared quantity; `'1'` is a
constant that happens to be certain. A property the type declares without a
default must be supplied, because no sensible stand-in exists for a quantity
that varies by orders of magnitude between deployments.

The shipped types are `client`, `load-balancer`, `queue`, `compute`,
`datastore`, and `aggregator`. Their properties, channels, and constraints are
listed in [the catalogue reference](../reference/catalogue.md), and a design may
[define its own](./component-types.md).

## Relationships

A relationship declares that one component calls another.

```yaml
# components/browsers.yaml
id: browsers
type: client
outgoing:
  - to: api
    summary: Checkout requests arriving at the API.
```

It is a wire, not a one-way pipe. Requests travel from `from` to `to` and the
response travels back along the same relationship, so a call graph is drawn once
rather than once per direction.

It is also a queue. Work offered faster than it can be taken waits somewhere,
and that somewhere is real whether or not anybody drew it: a socket buffer, a
listen backlog, a connection pool's wait list. Modelling the wire as a queue puts
that buffering in one place instead of asking every component type to
reimplement it.

```yaml
outgoing:
  - to: api
    capacity: '1'      # an in-process call, with nowhere to wait
```

`capacity` is how many operations may wait on the wire, and defaults to `100` —
the order of a network link between two services. Depth is not free. A queue
absorbs a burst by making the caller wait for it, so a generous buffer converts a
capacity problem into a latency one, and a caller with a deadline turns that
latency back into failure.

### Ports

A component type may declare several named places relationships attach. A
`compute` pool has one inbound port, `requests`, and one outbound port,
`dependencies`. A read-through cache might declare two outbound ports, one for
hits and one for misses, so the two paths can be sized apart instead of being
averaged into a figure that describes neither.

```yaml
outgoing:
  - to: orders
    from_port: misses
    to_port: operations
```

Both may be omitted when the type declares exactly one port on that side, which
is the common case and leaves simple designs free of wiring detail. A type with
several ports and a relationship that names none is an error rather than a
guess.

## Signals

A relationship carries named quantities called **signals**. The vocabulary is
small and each entry says how it behaves:

| Signal | Unit | Direction | Combines by |
| --- | --- | --- | --- |
| `rate` | `op/s` | forward | sum |
| `cancellation` | `op/s` | forward | sum |
| `occupancy` | `op` | forward | sum |
| `payload` | `B/op` | both | mean |
| `latency` | `s` | backward | max |
| `success` | `1` | backward | product |
| `capacity` | `op/s` | backward | min |

Two properties decide how the engine treats each one.

**How arrivals combine.** Request rates from several callers add together, but
the latency each of them observed does not; summing it would invent delay nobody
experienced. A component fanning out to several dependencies waits for the
slowest, so `latency` takes the maximum. Success multiplies, which treats every
dependency as hard: a component needing three services succeeds only when all
three do.

**Whether the quantity is shared out across replicas.** This is the extensive and
intensive distinction from physics. A `rate` divides across a sharded fleet; a
`payload` does not shrink because there are more shards to send it to. Getting
this wrong is quiet and expensive — treating a payload as extensive makes adding
shards look as though it shrinks records — so it is declared once per signal
rather than inferred.

Signals have no direction of their own. Which way one travels is settled by the
port publishing it: an inbound port publishes toward callers, an outbound port
toward dependencies. That is how `payload` names a request body in one place and
a reply body in the other without needing two names.

## Behaviours

A retry policy, a timeout, or a batching window is not a place work goes; it is a
rule about how work travels. Modelling them as components would put a box on the
diagram for every policy; modelling them as component properties would mean every
component type reimplementing the same rules.

A **behaviour** (a *mutator*, in the API) therefore sits on a relationship and
transforms the signals passing along it.

```yaml
outgoing:
  - to: api
    mutators:
      - type: retry
        properties:
          attempts: '3'
      - type: timeout
        properties:
          budget: '1'
```

The shipped behaviours are `retry`, `timeout`, `fan-out`, `batch`, `cache`,
`load-shed`, `feature-flag`, and `ignores-cancellation`.

### Order matters

Behaviours apply in the order they are declared, each transforming what the one
before it produced. A timeout *inside* a retry bounds each attempt; a timeout
*outside* one bounds the whole sequence, including the waiting between attempts.
Writing the order down makes that choice explicit instead of leaving it to be
inferred from prose.

### Amplification

The reason behaviours belong in a capacity model at all is that several of them
change how much demand arrives downstream:

- `retry` multiplies request rate by the expected number of attempts,
- `fan-out` multiplies it by the branch count,
- `batch` divides it while multiplying payload,
- `cache` reduces it to the miss rate.

Demand amplification is invisible in a diagram and is a common way for a design
to be wrong by an order of magnitude.

The retry policy reads the success rate coming *back* rather than taking a
constant, so its amplification rises exactly when the dependency starts failing.
That is a positive feedback loop — failing means more load, more load means
failing — and it is what turns a transient fault into a retry storm the system
cannot leave on its own.

## Shared quantities

Quantities the whole design refers to live in the **scratchpad** rather than
being repeated at every component that needs them.

```yaml
scratchpad:
  - name: peak_rate
    expression: '900'
    unit: op/s
    summary: Requests per second at the daily peak.
  - name: cache_hits
    expression: '0.5'
    unit: '1'
    summary: Share of reads served from cache.
  - name: pool_size
    expression: '8'
    unit: op
    summary: Concurrent requests one API replica serves.
```

A record size, a global request rate, or a peak-to-mean ratio is one fact about
the system. Stating it once means an experiment can change it once. Entries are
ordinary Squiggle bindings evaluated before anything else, in order, so a later
entry may build on an earlier one and any component may refer to any of them.

## Scale units

Large systems are not built by scaling every part independently. They are built
by designing a self-contained unit — a cell, a shard, a zone, a region — and
deploying many of them. A **scale unit** names that boundary, so a model
describes one unit and says how many exist.

```yaml
scale_units:
  - id: cell
    name: Serving cell
    replicas: '12'
    distribution: sharded
    members: [api, orders]
```

Constraints are evaluated per unit, which is the question worth asking. "Does one
cell have enough capacity" has an answer an engineer can act on; "does the fleet
have enough capacity in total" hides the cell that is hot while the average looks
fine.

Units nest, because real deployments do. A component's effective replica count is
the product along its chain of enclosing units, so a component inside ten shards
inside three regions is deployed thirty times. Set `parent` to nest one unit
inside another; a component claimed directly by two units is rejected.

`distribution` decides how demand meets those replicas, and it is a modelling
decision rather than a consequence of the count. `sharded` traffic divides, so
each replica serves its share. `mirrored` traffic does not: replicating writes to
every region means every region sees every write, so the count multiplies cost
without dividing load.

A component type's own `replicas` property is a different statement. It
replicates *one* component behind a shared entry point, where a scale unit
replicates a *set* of components together as a deployable whole. A pool of
servers is the former; a cell containing a pool, its queue, and its store is the
latter.

## Interventions

Comparing two designs only means something if the two are otherwise identical.
Editing a model to try an idea destroys that guarantee: the before and after
differ by whatever was changed plus whatever was disturbed along the way.

An intervention therefore changes nothing structural. It rebinds named
quantities in the scratchpad, and the model is solved again exactly as it stands.

```yaml
interventions:
  - id: warm-cache
    name: Warm the cache
    summary: Raise the hit ratio by holding a larger working set.
    overrides:
      - name: cache_hits
        expression: '0.95'
```

That constraint is also a design discipline. Expressing an idea as an
intervention forces the quantity it acts on to have been named in the first
place, which is usually where the thinking is. "Add a cache" is not a proposal
until it becomes "the hit ratio becomes 0.9", and the second is something an
engineer can argue with.

Because scratchpad entries may refer to earlier ones, rebinding an early quantity
carries through everything derived from it without any of those components being
mentioned.

A replacement is an ordinary expression and may depend on time, so a change that
arrives gradually is written as one:

```yaml
overrides:
  - name: peak_rate
    expression: 'if t < 300 then 900 else 3600'
```

The quantity was always a function of time; a constant was only the simplest
case.

## Choosing how much detail

Start with the smallest design that can answer the question:

1. Put a `client` at the boundary and give it the objective the system exists to
   meet. Latency and success propagate back to it, so its constraints report on
   the whole design rather than on any one hop.
2. Add the components on the path a request actually takes.
3. Add a behaviour only where it changes demand, latency, or success — a retry,
   a timeout, a cache, a fan-out.
4. Name a quantity in the scratchpad as soon as two components need it, or as
   soon as you want to vary it.
5. Add a scale unit when the answer should describe one cell rather than the
   fleet.
6. Add an intervention for each proposal you intend to weigh.

A denser model is not a better one. Every property should be something an
engineer can measure, look up, or defend, and the `summary` field beside it is
where that defence belongs.
