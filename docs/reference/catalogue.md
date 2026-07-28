# Shipped catalogue

The vocabulary is deliberately small. Each entry covers a role that recurs in
nearly every system design, and anything more specialised is better expressed as
a [project-local type](../guide/component-types.md) than as a catalogue entry
nobody else can use.

The catalogue is embedded in the binary and validated on load, and project-local
definitions are loaded alongside it and checked by identical rules. Nothing here
is privileged, and a design may replace any of it.

```sh
optimist catalogue ./design --output json
```

## Signals

The quantities that travel along a relationship. A signal has no direction of its
own: which way it travels is settled by the port publishing it.

| Signal | Unit | Combines by | Extensive | Meaning |
| --- | --- | --- | --- | --- |
| `rate` | `op/s` | sum | yes | Operations per second travelling along the relationship. |
| `cancellation` | `op/s` | sum | yes | Operations the caller has abandoned. Travels forward, because only the caller knows it has given up. |
| `occupancy` | `op` | sum | yes | Operations in flight: work a caller is holding open against a dependency. |
| `payload` | `B/op` | mean | no | Bytes carried by one operation. Read on an inbound port it is the request; on an outbound port it is the reply. |
| `latency` | `s` | max | no | Time from receiving a request to answering it, including every downstream call. Travels back to the caller. |
| `success` | `1` | product | no | Probability a request is answered successfully. Travels back to the caller. |
| `capacity` | `op/s` | min | yes | Rate the callee can sustain, reported back so the wire in front of it knows how fast it drains. |

**Extensive** means the quantity is shared out across the replicas of a scale
unit. A rate divides across a sharded fleet; a payload does not shrink because
there are more shards to send it to.

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
| `success_target` | `1` | `0` |

**Channels** — `offered`, `latency`, `success`, `failure`.

**Constraints** — `latency_objective` (observed latency against the target),
`success_objective` (observed failure rate against what the objective allows).

### `load-balancer`

Spreads demand across replicas and refuses what it cannot admit. The only
component that can pull a saturated system back out of congestion, because it
acts on demand rather than on how long a request waits.

A connection is held for the whole round trip, so backends slowing down consumes
its connection limit without any change in demand at all.

**Ports** — `in.requests` publishes `latency`, `success`, `capacity`;
`out.backends` publishes `rate`, `cancellation`.

| Property | Unit | Default |
| --- | --- | --- |
| `admission_limit` | `op/s` | *required* |
| `connection_limit` | `op` | *required* |
| `replicas` | `1` | `1` |
| `overhead` | `s` | `0` |

**Channels** — `arriving`, `cancelled`, `propagated_cancellation`, `offered`,
`admitted`, `shed`, `per_replica`, `backend_wait`, `backend_success`, `latency`,
`connections`, `success_rate`.

**Constraints** — `admission` (offered against the admission limit),
`connections` (connections held against the limit).

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

**Channels** — `arrivals`, `departures`, `backlog`, `accepted_ratio`, `wait`.

`backlog` reads `prev.backlog`, so this type is a state variable and its
behaviour under [transient solving](../guide/analysis.md#steady-state-and-transient)
differs from its steady state.

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
| `replicas` | `1` | `1` |
| `request_size` | `B/op` | `0` |

**Key channels**

| Channel | Expression |
| --- | --- |
| `hold_time` | `service_time + dependency_wait` |
| `servers` | `parallelism * replicas` |
| `capacity` | `Little.rate(servers, hold_time)` |
| `utilisation` | `Queue.utilisation(offered, capacity)` |
| `held_downstream` | `Little.occupancy(calls, dependency_wait)` |

Also `arriving`, `cancelled`, `salvaged`, `offered`, `answered`,
`propagated_cancellation`, `dependency_wait`, `dependency_success`, `residence`,
`concurrency`, `calls`, `success_rate`.

**Constraints** — `capacity` (offered load against sustainable throughput).

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

**Channels** — `arriving`, `cancelled`, `operations`, `held`, `concurrency`,
`latency`, `records`, `volume`, `transfer`, `replicated`, `utilisation`,
`success_rate`.

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

**Channels** — `arriving`, `cancelled`, `fanned_out`, `propagated_cancellation`,
`branch_capacity`, `branch_wait`, `branch_success`, `latency`, `success_rate`.

**Constraints** — `fan_out` (demand created against demand received).

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
| `hit_ratio` | `1` | *required* |

Request `rate` becomes `signal.rate * (1 - hit_ratio)`. Uncertainty here should
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
| `exposure` | `1` | *required* |

Request `rate` becomes `signal.rate * min([max([exposure, 0]), 1])`. Point
`exposure` at a shared quantity so an intervention can move it without touching
the structure of the model, and give two connections complementary shares to
model a routed rollout.

A flag at zero starves everything behind it, which makes it useful for asking
whether a component is needed at all: the rest of the design is solved exactly as
it stands, with that path carrying nothing.

### `ignores-cancellation`

Drops cancellation on its way downstream, so work the caller has abandoned
carries on being done. Models a hop with no cancellation plumbing: a call made
without a context to propagate, a client that closes its connection without
aborting the request behind it, a queue entry nobody withdraws.

It takes no properties. Request `cancellation` becomes `0`.

This is the behaviour that separates a system which degrades from one that
collapses. A timeout above a retry normally protects the dependency twice over:
it bounds the wait and it withdraws the work, and the withdrawn load is what lets
a saturated dependency climb back out. Remove the withdrawal and only the bound
remains, so the caller gives up, retries, and the dependency now serves both the
abandoned attempt and its replacement. Load rises while useful work falls, which
is exactly the shape of a system that stays down after the thing that knocked it
over has passed.
