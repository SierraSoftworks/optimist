# Shipped catalogue

The vocabulary is deliberately small. Each entry covers a role that recurs in
nearly every system design, and anything more specialised is better expressed as
a [project-local type](../guide/component-types.md) than as a catalogue entry
nobody else can use.

The catalogue is embedded in the binary and validated on load, and project-local
definitions are loaded alongside it and checked by identical rules. Nothing here
is privileged, and a design may replace any of it.

```sh
optimist catalogue ./design                # what is available, and what is used
optimist catalogue ./design --type compute # one entry, in full
optimist catalogue ./design --output json
```

`--type` is the quickest way to see which of an entry's properties are required,
what unit each carries, and which limits it can exhaust — worth reading before
writing a component rather than after it fails to solve.

## Signals

The quantities that travel along a relationship. A signal has no direction of its
own: which way it travels is settled by the port publishing it.

| Signal | Unit | Combines by | Extensive | Meaning |
| --- | --- | --- | --- | --- |
| `rate` | `op/s` | sum | yes | Operations per second travelling along the relationship. |
| `cancellation` | `op/s` | sum | yes | Operations the caller has abandoned. Travels forward, because only the caller knows it has given up. |
| `cancellation_effectiveness` | `share` | mean | no | Share of those cancellations arriving in time to save the work. Rests at a half rather than at an aggregation identity. |
| `occupancy` | `op` | sum | yes | Operations in flight: work a caller is holding open against a dependency. |
| `payload` | `B/op` | mean | no | Bytes carried by one operation. Read on an inbound port it is the request; on an outbound port it is the reply. |
| `latency` | `s` | max | no | Time from receiving a request to answering it, including every downstream call. Travels back to the caller. |
| `success` | `share` | product | no | Probability a request is answered successfully. Travels back to the caller. |
| `capacity` | `op/s` | min | yes | Rate the callee can sustain, reported back so the wire in front of it knows how fast it drains. |
| `peers` | `1` | sum | no | Nodes on the far end: how many replicas of the peer one replica of this component talks to. Supplied by the engine, not published by a component. |

`peers` is the one signal no component writes. A component cannot see its own
surroundings, so the engine states how many replicas of the peer sit on the far
end of each relationship, with any scale unit enclosing *both* ends divided out —
units deployed together are deployed together, and a shard's writer talks to the
one store inside its own shard rather than to every shard's. That is what lets a
[`quorum`](#quorum) read its group size from the deployment instead of having it
restated as a property that can drift out of step with the scale unit beside it.

**Extensive** means the quantity is shared out across the replicas of a scale
unit. A rate divides when it enters a sharded fleet and gathers when it leaves;
capacity reported by the fleet gathers on the way back. Relationships within
one unit stay local, and a payload does not shrink because there are more shards
to send it to.

A signal reads its aggregation's identity where nothing arrives carrying it —
nought for a rate, one for a success, no ceiling for a capacity. A signal may
state a different resting value where it describes a convention rather than a
flow, which is how `cancellation_effectiveness` assumes a half without any
behaviour having to say so.

A signal a manifest introduces without declaring falls back to adding across
arrivals and not dividing across replicas. That is the safe reading: it may
overstate the load on one replica, and overstating a bottleneck is recoverable in
a way that missing one is not.

---

## Component types

### `client`

A population of callers offering work to the system. Has no capacity of its own:
it is where demand enters a model and where the answer arrives back.

Because responses propagate back to whoever made the call, this is also the
measurement point. The latency and success it reports are what a user experiences
end to end, with every hop, retry, timeout, and fan-out already folded in.
Declaring a target here turns "does this design meet its objective" into a
constraint the engine can rank.

**Ports** — `out.calls` publishes `rate`, `payload`.

| Property | Unit | Default |
| --- | --- | --- |
| `request_rate` | `op/s` | *required* |
| `payload` | `B/op` | `0` |
| `latency_target` | `s` | `infinity` |
| `success_target` | `share` | `0` |

**Channels** — `offered`, `latency`, `success`, `failure`.

**Constraints** — `latency_objective` (observed latency against the target),
`success_objective` (observed failure rate against what the objective allows).

### `load-balancer`

Fronts a backend fleet, forwarding demand to it and holding a connection open
for every request in flight. A sharded scale unit around the backends divides
forwarded demand between their replicas.

A connection is held for the whole round trip, so backends slowing down consumes
its connection limit without any change in demand at all.

Refusing work is not a property of this component. Put an explicit `load-shed`
behaviour on the relationship in front when the design deliberately refuses
demand.

**Ports** — `in.requests` publishes `latency`, `success`, `capacity`;
`out.backends` publishes `rate`, `cancellation`.

| Property | Unit | Default |
| --- | --- | --- |
| `connection_limit` | `op` | *required* |
| `overhead` | `s` | `0` |

**Channels** — `arriving`, `cancelled`, `propagated_cancellation`, `offered`,
`backend_capacity`, `forwarded`, `backend_wait`, `backend_success`, `latency`,
`connections`, `success_rate`.

**Constraints** — `connections` (connections held against the limit).

`backend_capacity` is the rate the fleet sustains and the balancer publishes to
callers, because the balancer holds no work of its own. `forwarded` is everything
it was offered; put replicated backends in a sharded scale unit to divide it
between them. Overload is charged once, by the queue on the wire between the
balancer and its backends, so `success_rate` reports what the backends managed
rather than restating that shortfall.

### `failover`

Splits demand between two independent backends and shifts it away from the
primary as the primary's health falls. The two legs are separate designs with
their own capacity, so moving work between them is a real redistribution: what
the primary stops taking, the standby starts.

Distinct from replication inside a scale unit. A scale unit's replicas are copies
of one design, and losing some of them is a loss of capacity in that unit, which
is what a smaller replica count says. This type is for the other case, where the
alternative is somewhere else entirely: another region, an older version, a
different provider.

**Ports** — `in.requests` publishes `latency`, `success`; `out.primary` and
`out.standby` each publish `rate`, `cancellation`. Both outbound ports are
required, because an empty port answers instantly and without fail and a standby
that does not exist would make failing over look free.

| Property | Unit | Default |
| --- | --- | --- |
| `primary_weight` | `share` | `1` |
| `latency_threshold` | `s` | `1e9` |
| `success_threshold` | `share` | `0` |
| `overhead` | `s` | `0` |

**Channels** — `arriving`, `cancelled`, `propagated_cancellation`, `offered`,
`primary_latency`, `primary_success`, `latency_health`, `success_health`,
`primary_health`, `primary_share`, `standby_share`, `to_primary`, `to_standby`,
`primary_cancellation`, `standby_cancellation`, `standby_latency`,
`standby_success`, `latency`, `success_rate`.

**Constraints** — none. The router has no limit of its own; the legs report
theirs.

`primary_weight` is the split while the primary is healthy: one is a true
active/standby pair, a half splits evenly. It may be written against `t`, which
is how a progressive rollout is expressed — the weight becomes a schedule and the
solve reports what each stage of it costs. `primary_health` is the worse of the
two thresholds and is continuous rather than a step, because a health check
ejects endpoints one at a time as a backend degrades and because a step would
leave the relaxation oscillating between two answers. `latency` and
`success_rate` are blended at the share each leg carries rather than taken as the
worse of them, because a request goes to one backend or the other; failing over
is therefore only as good as what it fails over to.

### `queue`

A buffer that decouples a producer's arrival rate from a consumer's service
capacity. Absorbs bursts up to its depth and converts sustained overload into
growing backlog and waiting time rather than immediate rejection.

Unlike a synchronous call, the waiting is not spent by the producer. A producer
that hands work to a queue is answered as soon as the work is accepted, so the
delay travels onward to the consumer as staleness rather than back to the
producer as latency. That is the whole point of a queue, and it is also why a
queue cannot relieve a shortfall in consumer capacity: it changes who waits, not
how much work there is.

**Ports** — `in.work` publishes `success`, `latency`; `out.consumers` publishes
`rate`, `latency`.

| Property | Unit | Default |
| --- | --- | --- |
| `service_rate` | `op/s` | *required* |
| `capacity` | `op` | *required* |

**Channels** — `arrivals`, `departures`, `load`, `backlog`, `accepted_ratio`,
`wait`.

`backlog` and `accepted_ratio` have two forms. Asked where the design rests they
are the stationary M/M/1/K results for this depth at this load — the same law the
buffer on every relationship uses, so the two agree — and the answer does not
depend on the solver's step. Asked how the design moves they integrate from
`prev.backlog`, which is what gives the type memory and makes recovery time
visible under [transient solving](../guide/analysis.md#steady-state-and-transient).
Only accepted work accumulates, and the total is bounded by the depth.

**Constraints** — `depth` (backlog against the queue's depth), `throughput`
(arrivals against the rate consumers drain at).

### `compute`

A pool of identical workers serving requests. Capacity follows from how many
requests can be in flight at once and how long each occupies a worker, so raising
parallelism and shortening service time are interchangeable levers on throughput
but not on latency.

A worker is held for the whole of a request, including any time spent waiting on
a dependency, so this pool's capacity depends on how fast the things it calls
are. That coupling is what allows a slow dependency to saturate a caller that is
itself doing very little work.

**Ports** — `in.requests` publishes `latency`, `success`, `capacity`;
`out.dependencies` publishes `rate`, `occupancy`, `cancellation`, `payload`.

| Property | Unit | Default |
| --- | --- | --- |
| `service_time` | `s` | *required* |
| `parallelism` | `op` | *required* |
| `request_size` | `B/op` | `0` |

**Key channels**

| Channel | Expression |
| --- | --- |
| `hold_time` | `service_time + dependency_wait` |
| `servers` | `parallelism` |
| `capacity` | `Little.rate(servers, hold_time)` |
| `utilisation` | `Queue.utilisation(offered, capacity)` |
| `held_downstream` | `Little.occupancy(calls, dependency_wait)` |

Also `arriving`, `cancelled`, `salvage_share`, `salvaged`, `offered`,
`propagated_cancellation`, `dependency_wait`, `dependency_success`, `residence`,
`concurrency`, `calls`, `success_rate`.

**Constraints** — `capacity` (offered load against sustainable throughput).

These figures describe one replica. Put the component in a sharded scale unit
to deploy several replicas and divide the arriving demand between them.

`success_rate` reports what this pool's dependencies managed, and nothing else.
Neither the work refused by the queue in front of it nor the work its caller
withdrew appears there: both are counted where they happened, and restating
either would charge one failure twice — and once more for every further hop a
cancellation reached.

Asked for more than it can serve, this pool does not quietly serve less. It
reports the share it could not answer and the caller decides what to do about it.
Refusing work is a design decision that belongs on a relationship, as an explicit
`load-shed` behaviour, rather than an unannounced property of every component
that happens to be busy.

### `datastore`

Durable storage with independent limits on operation rate, transfer rate,
resident volume, and simultaneous work. Reaching any one of them bottlenecks the
system, and which one binds first depends on record size rather than on the store
itself.

Latency here is not a fixed property. It rises with how hard the store is being
driven, and because that delay travels back to whoever called, a store near its
ceiling consumes the capacity of every service waiting on it.

**Ports** — `in.operations` publishes `latency`, `success`, `capacity`,
`payload`; `out.replication` publishes `rate`, `payload`.

| Property | Unit | Default |
| --- | --- | --- |
| `operation_limit` | `op/s` | *required* |
| `transfer_limit` | `B/s` | *required* |
| `volume_limit` | `B` | *required* |
| `record_size` | `B/op` | *required* |
| `retention` | `s` | *required* |
| `concurrency_limit` | `op` | `infinity` |
| `service_time` | `s` | `0.001` |
| `replication_factor` | `1` | `0` |

**Channels** — `arriving`, `cancelled`, `salvaged`, `operations`, `held`,
`concurrency`, `latency`, `records`, `volume`, `transfer`, `replicated`,
`utilisation`, `success_rate`.

**Constraints** — `operations`, `transfer`, `volume`, `concurrency`.

Retention is the one people forget. `volume = Little.occupancy(operations,
retention) * record_size`, so thirty days of a modest write rate is often the
binding constraint in a design where everybody is watching the compute pool.

### `aggregator`

Fans one request out across several branches and combines what comes back. A
request completes only when every branch has, so this is where reliability and
tail latency are decided rather than where capacity is.

Both effects work against the caller at once. Waiting for the slowest of many
branches makes the typical response as slow as an unusual one, and needing all of
them to succeed multiplies their failure rates together. Adding a branch is never
free, even when the branch itself is fast and reliable.

**Ports** — `in.requests` publishes `latency`, `success`, `capacity`;
`out.branches` publishes `rate`, `cancellation`.

| Property | Unit | Default |
| --- | --- | --- |
| `branches` | `1` | *required* |
| `overhead` | `s` | `0` |

**Channels** — `arriving`, `cancelled`, `salvaged`, `fanned_out`,
`propagated_cancellation`, `branch_capacity`, `branch_wait`, `branch_success`,
`latency`, `success_rate`.

**Constraints** — `fan_out` (demand created against demand received).

`branch_capacity` divides what the slowest branch can take by the number of calls
each request makes of it. A branch reporting no ceiling, and a fan-out with
nothing wired to it yet, both arrive as an unbounded figure; it is capped at a
rate no real service reaches before being divided, so a design part-way through
being drawn still solves.

### `quorum`

Sends every request to each node of a replicated group and answers as soon as a
strict majority has replied. Consensus stores, replicated logs, and leader
elections are all built this way, and the reason is that a majority is the
smallest set that cannot be assembled twice at once.

This is the arrangement that inverts the arithmetic of a fan-out. Waiting for all
of several nodes makes a group slower and less reliable than any one of them;
waiting for most of them makes it faster and more reliable than any one of them,
because the slowest and the failed are exactly the ones a majority leaves behind.
It is the only entry in the catalogue where adding a dependency improves a
design.

What it does not do is reduce load. Every node still receives every request, so a
quorum costs what a fan-out costs and buys latency and availability with it
rather than throughput.

**Ports** — `in.requests` publishes `latency`, `success`, `capacity`;
`out.members` publishes `rate`, `cancellation`.

| Property | Unit | Default |
| --- | --- | --- |
| `overhead` | `s` | `0` |

**Channels** — `arriving`, `cancelled`, `salvaged`, `nodes`, `quorum`,
`replicated`, `propagated_cancellation`, `issued`, `node_capacity`, `node_wait`,
`quorum_wait`, `node_success`, `latency`, `success_rate`.

**Constraints** — `replication` (node requests issued against those arriving).

The group's size is not authored. Attach **one** member and put it in a mirrored
scale unit whose replica count is the number of nodes; `nodes` reads that count
through the [`peers`](#signals) signal and `quorum` is `floor(n / 2) + 1`. A
member with no scale unit around it is a group of one, which is what a single
node is.

`out.members` takes exactly one relationship, and a second is refused rather than
combined. The figures the type reads are one node's; `success` multiplies across
arrivals, so a second relationship would hand it the chance that *every* node
succeeded where it wanted the chance that one did, and report a healthy group as
a failing one.

An even group is worth noticing: four nodes await three replies, exactly as five
do, so the fourth costs a node to run and a node's availability to lose while
buying nothing.

---

## Behaviours

Behaviours attach to a relationship and apply in the order they are declared.

### `retry`

Re-issues a failed call up to a fixed number of attempts. Raises the chance a
call eventually succeeds, and raises the demand placed on the dependency by the
same mechanism, which is why a retry policy helps a healthy system and harms a
failing one.

| Property | Unit | Default |
| --- | --- | --- |
| `attempts` | `1` | *required* |

| Direction | Signal | Expression |
| --- | --- | --- |
| request | `rate` | `signal.rate * Reliability.retryAttempts(response.success, attempts)` |
| response | `success` | `Reliability.retrySuccess(signal.success, attempts)` |
| response | `latency` | `signal.latency * Reliability.retryAttempts(signal.success, attempts)` |

The amplification reads the success rate coming *back*, so it rises exactly when
the dependency starts failing. That is a positive feedback loop, and it is what
turns a transient fault into a retry storm the system cannot leave on its own.

This policy knows nothing about time. It retries what failed; whether a slow
answer counts as a failure is a separate decision expressed by placing a
`timeout` beneath it.

### `timeout`

Abandons a call that has not returned within a deadline. Bounds the latency a
caller can observe and converts the tail it cut off into failures rather than
removing it from the system.

| Property | Unit | Default |
| --- | --- | --- |
| `budget` | `s` | *required* |

| Direction | Signal | Effect |
| --- | --- | --- |
| request | `cancellation` | Adds the calls whose deadline passed before an answer arrived. |
| response | `latency` | Capped per draw at the budget. |
| response | `success` | Reduced by the share that had not answered in time. |

Latency is clamped rather than discarded, because a call that timed out still
occupied a connection for the full budget. The share of draws sitting exactly at
the budget is the share that timed out.

Placed beneath a `retry` it bounds each attempt; placed above one it bounds the
whole sequence, because the cancellation it raises travels forward and withdraws
the work still in flight.

### `fan-out`

Turns one upstream call into several downstream ones. The commonest source of
demand a design underestimates, because the multiplier lives in application code
rather than anywhere a diagram shows.

| Property | Unit | Default |
| --- | --- | --- |
| `branches` | `1` | *required* |

Request `rate` becomes `signal.rate * branches`.

### `batch`

Groups several calls into one. Trades operation rate for payload size and waiting
time, which is the right trade against a store limited by operations per second
and the wrong one against a store limited by bandwidth.

| Property | Unit | Default |
| --- | --- | --- |
| `size` | `1` | *required* |
| `max_delay` | `s` | `0` |

Request `rate` is divided by `size` and `payload` multiplied by it, so the byte
rate through the relationship is unchanged while the operation rate falls.
Response `latency` gains `max_delay`.

### `cache`

Serves a share of calls without reaching the dependency. Only the misses travel
on, so the dependency sees demand reduced by the hit ratio while the caller still
sees every call.

| Property | Unit | Default |
| --- | --- | --- |
| `hit_ratio` | `share` | *required* |

Request `rate` becomes `signal.rate * (1 - hit_ratio)`, with the ratio clamped
into zero and one so that a mistyped setting cannot send negative demand
downstream. Uncertainty here should
reflect how much the working set may change: a hit ratio measured on today's
traffic is a poor guide to tomorrow's, and a design that depends on a high one
should be checked against a low one.

### `load-shed`

Refuses demand above a fixed rate. The only shipped behaviour that acts on demand
rather than on how long a call waits, which is what makes it the one lever capable
of pulling a congested system back out of congestion.

| Property | Unit | Default |
| --- | --- | --- |
| `limit` | `op/s` | *required* |

Request `rate` is capped per draw at the limit, and response `success` is reduced
by the share refused. Shedding is not free and not invisible: without that second
term the model would report a shedding system as almost perfectly successful
while it refused most of its traffic.

Setting the limit below the downstream capacity is the point. A limit above
capacity sheds nothing until the dependency has already saturated.

### `feature-flag`

Admits a share of traffic along a connection, so part of a design can be turned
off, turned on, or exposed to a fraction of requests.

| Property | Unit | Default |
| --- | --- | --- |
| `exposure` | `share` | *required* |

Request `rate` becomes `signal.rate * min(max(exposure, 0), 1)`. Point
`exposure` at a shared quantity so an intervention can move it without touching
the structure of the model, and give two connections complementary shares to
model a routed rollout.

A flag at zero starves everything behind it, which makes it useful for asking
whether a component is needed at all: the rest of the design is solved exactly as
it stands, with that path carrying nothing.

### `fallible`

Carries calls across a link that can independently lose requests and replies.

| Property | Unit | Default |
| --- | --- | --- |
| `transmit_failure` | `share` | `0` |
| `receive_failure` | `share` | `0` |

The two losses are not interchangeable. A request lost while transmitting never
reaches the dependency, so it fails the caller *and* relieves downstream demand.
A reply lost on the way back leaves the dependency's work done and paid for, so
it fails the caller alone. Both shares are clamped into zero and one, because a
probability outside that range has no physical meaning.

### `ignores-cancellation`

Drops cancellation on its way downstream, so work the caller has abandoned
carries on being done. Models a hop with no cancellation plumbing: a call made
without a context to propagate, a client that closes its connection without
aborting the request behind it, a queue entry nobody withdraws.

It takes no properties. Request `cancellation` becomes `0`, which is the same as
attaching `cancellation-effectiveness` at nought.

This is the behaviour that separates a system which degrades from one that
collapses. A timeout above a retry normally protects the dependency twice over:
it bounds the wait and it withdraws the work, and the withdrawn load is what lets
a saturated dependency climb back out. Remove the withdrawal and only the bound
remains, so the caller gives up, retries, and the dependency now serves both the
abandoned attempt and its replacement. Load rises while useful work falls, which
is exactly the shape of a system that stays down after the thing that knocked it
over has passed.

### `cancellation-effectiveness`

Says how much of the work a cancellation withdraws is actually saved at the far
end.

| Property | Unit | Default |
| --- | --- | --- |
| `effectiveness` | `share` | `0.5` |

Request `cancellation_effectiveness` becomes the value clamped into zero and one.
Without it the share is a half, on the assumption that a cancellation is equally
likely to land at any point during a request — the right guess in the absence of
anything better, and the wrong one wherever a design knows more. A hop that
checks for cancellation before starting work saves nearly all of it; one that
checks only before replying saves none.

The share decides load, not success. A request the caller gave up on has already
failed that caller, and the behaviour that gave up is what reports it.

### `message-size`

States how large the request and the reply are on a connection, so the bytes
crossing it can be measured against the link's speed.

| Property | Unit | Default |
| --- | --- | --- |
| `request_size` | `B/op` | `0` |
| `response_size` | `B/op` | `0` |

Request `payload` becomes `request_size` and response `payload` becomes
`response_size`. Sizes belong on the connection rather than on either end of it:
the same store answers a key lookup and a full document scan, and which it is was
decided by the caller.

Reply size matters more often than request size and is left out more often. A
chatty API returning whole objects to callers that wanted one field is a
bandwidth problem that looks like a latency problem.

---

## Relationships

A relationship is a wire, and a wire has limits of its own that belong to neither
end.

| Property | Unit | Default |
| --- | --- | --- |
| `capacity` | `op` | `100` |
| `bandwidth` | `B/s` | unlimited |

`capacity` is how many operations may wait on the wire: socket buffers and a
listen backlog for a network hop, nearer one for an in-process call, far larger
for a broker with disk backing.

`bandwidth` is how fast it carries bytes, against the request and reply payloads
together. It is reported as a `bandwidth` constraint on the relationship, and
only where a speed was stated — a link nobody gave a speed to is not a link that
is full. This is the limit an operation-rate model cannot report at all: a design
whose rates all fit comfortably can still be bound by the bytes those operations
move, and reading it sends somebody to the network rather than to the service.
