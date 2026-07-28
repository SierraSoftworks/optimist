# Writing component types

The evaluator reads a component type to discover what to compute; it never learns
what a queue or a datastore *is*. Adding a new kind of component is therefore a
matter of writing a manifest, not of changing the engine, and a design may
introduce kinds the catalogue never anticipated.

Project-local definitions live beside the design and are loaded over the shipped
catalogue:

```text
examples/metastable/
  _system.yaml
  components/
  component-types/connection-pool.yaml
  mutators/hedged-request.yaml
```

They are checked by identical rules, and nothing shipped is privileged — a
definition may replace a catalogue type it disagrees with as well as add one
nobody anticipated.

## Anatomy of a type

A type declares four things.

```yaml
id: token-bucket
name: Token bucket
summary: >
  Admits requests against a refilling allowance, so a burst is served from the
  bucket and a sustained excess is refused.

ports:
  in:
    requests:
      arity: many
      summary: Callers arriving at the limiter.
      publishes:
        success: admitted_ratio
        latency: '0'
  out:
    downstream:
      arity: one
      summary: The service this limiter protects.
      publishes:
        rate: admitted

properties:
  refill:
    unit: op/s
    summary: Tokens added per second, and the sustained rate admitted.
  burst:
    unit: op
    summary: Tokens the bucket holds, and therefore the burst it absorbs.
    default: '0'

channels:
  offered:
    unit: op/s
    summary: Demand arriving from every caller.
    expression: in.requests.rate
  admitted:
    unit: op/s
    summary: Demand passed on, capped per draw at the refill rate.
    expression: min([offered, refill])
  admitted_ratio:
    unit: '1'
    summary: Share of callers served rather than refused.
    expression: min([admitted / max([offered, 0.000001]), 1])

constraints:
  throughput:
    summary: >
      Offered load against the sustained allowance. Saturating means callers are
      being refused, which is a cost paid deliberately.
    demand: offered
    limit: refill
```

**Properties** are the intrinsic facts an author supplies, each carrying a unit
annotation. A property without a `default` must be supplied.

**Channels** are quantities derived from properties, from the flows arriving on
inbound relationships, and from the component's own state at the previous step.
Each is a Squiggle expression evaluated over sample sets, so uncertainty flows
through untouched. Channels are ordered automatically by what they refer to; a
cycle *within* one component is rejected.

**Ports** are the named places relationships attach, and what each publishes onto
them. An inbound port publishes the response sent back to callers, so only
signals that travel backward may appear there. An outbound port publishes the
request sent to dependencies, so only signals that travel forward may.

**Constraints** pair a demand channel with the limit it consumes, and are the
whole point of the exercise. Every bottleneck the engine reports is a constraint
whose demand has approached its limit.

The engine attaches no meaning to any particular name. `throughput` is whatever a
manifest says it is, and a constraint called `iops` is ranked by exactly the same
arithmetic as one called `bandwidth`.

## What an expression may refer to

The surface is deliberately small. A channel expression may use:

| Name | Meaning |
| --- | --- |
| *property names* | This component's own properties. |
| *channel names* | Other channels on this component. |
| `in.<port>.<signal>` | What arrived on an inbound port, aggregated across callers. |
| `out.<port>.<signal>` | What came back on an outbound port, aggregated across dependencies. |
| `prev.<channel>` | This component's channel values at the previous step. |
| `t` | Elapsed seconds since the run began. |
| `dt` | Length of the current step, in seconds. |
| *scratchpad names* | Shared quantities, when used in a component's properties. |
| Squiggle builtins | The whole standard library, including the queueing namespaces. |

A component cannot see what it publishes itself. `in.<port>` is what arrived and
`out.<port>` is what came back, so neither can be confused for the response the
component is sending or the request it is making. That is what keeps a component
from depending on what it is saying.

Every expression is parsed and its free identifiers collected when the catalogue
loads, then checked against that surface. A mistyped name fails at load rather
than when a solver is midway through a run.

## Modelling with `prev`

`prev` is what gives a design memory, and it is only meaningful under
[transient solving](./analysis.md#steady-state-and-transient). The shipped
`queue` type uses it to accumulate a backlog:

```yaml
channels:
  backlog:
    unit: op
    summary: >
      Work resident in the queue, accumulated from the imbalance between
      arrivals and departures and floored at zero.
    expression: max([prev.backlog + (arrivals - departures) * dt, 0])
```

A channel that reads `prev` is a state variable. Without one, a component is a
pure function of its inputs and the design has no history to recover from.

## Writing a behaviour

A behaviour transforms the signals travelling along a relationship. It sees the
flow and its own settings, never the components on either end, which is what lets
one definition apply to any connection.

```yaml
id: hedged-request
name: Hedged request
summary: >
  Issues a second request once the first has been outstanding longer than the
  hedge delay, taking whichever answers first.
properties:
  hedge_after:
    unit: s
    summary: Delay before a duplicate is issued.
  hedge_share:
    unit: '1'
    summary: Share of calls slow enough to be hedged.
requests:
  rate:
    unit: op/s
    summary: Demand raised by the share of calls that are duplicated.
    expression: signal.rate * (1 + hedge_share)
responses:
  latency:
    unit: s
    summary: Waiting bounded by the hedge delay plus one service time.
    expression: min([signal.latency, hedge_after + signal.latency * (1 - hedge_share)])
```

`requests` rewrites signals on the way downstream — what the caller is asking its
dependency for. `responses` rewrites them on the way back — what the dependency
reports to its caller. A timeout belongs in `responses`, because it bounds what
the caller waits for and turns the tail it cut off into failure rather than
changing what the dependency was asked to do.

A transform may refer to:

| Name | Meaning |
| --- | --- |
| *property names* | This behaviour's own settings. |
| `signal.<name>` | The flow this transform is rewriting, as it arrived. |
| `request.<name>` | The request travelling from caller to callee. |
| `response.<name>` | The response returning from callee to caller. |
| `t`, `dt` | Elapsed time and step length. |

Both directions are visible from either transform. That is deliberate: it is what
lets a retry policy raise demand precisely when the response says attempts are
failing.

`signal` always holds the flow as it arrived at this behaviour, before any of the
transforms in the same definition ran, so transforms within one behaviour are
order-independent. Behaviours attached to the same relationship are not; they
compose in declaration order.

## Rules a definition must satisfy

Load-time validation rejects a definition when:

- the identifier is not lower-case words joined by hyphens,
- a property or channel name is not a usable Squiggle binding,
- one name is declared as both a property and a channel,
- an expression does not parse,
- an expression refers to a name the evaluator will not supply,
- a unit annotation does not parse,
- a port publishes a quantity that is neither a property nor a channel.

Unknown fields are refused, in a manifest exactly as in a design document. A
file that nearly parses is more dangerous than one that does not: a manifest
naming a section the engine has since renamed would otherwise load with that
section missing, then solve and report plausible numbers that are wrong wherever
it would have carried a flow.

## Two things worth knowing before you start

**Response ports and `rate`.** Signals flow along a relationship in one
direction only, and an outbound port's `publishes` applies to every relationship
leaving it. A component sitting on the response leg of a feedback loop that
publishes `rate` feeds demand back into its own caller, which gives the loop a
gain above one and no fixed point to find. If a type exists to model a delay on
the way back, publish `latency` and leave `rate` alone.

**Choose the state variable carefully.** A component whose utilisation is derived
from an arrival *rate* cannot produce an occupancy-driven fold, because there is
nothing in it that remembers. If you are trying to reproduce bistability, the
component needs a channel that reads `prev` and a constraint that reads that
channel.

## Checking your work

```sh
cargo run -- catalogue ./design
```

```text
KIND       ID             PROPERTIES  LIMITS
component  token-bucket   2           1
behaviour  hedged-request 2           2
```

`catalogue` lists everything a design can reach, shipped and local together, and
`--output json` prints the full manifests including every summary. `check` loads
the same definitions and reports the first that fails validation.
