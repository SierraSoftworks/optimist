# NALSD modelling techniques

A component catalogue says what Optimist can model. This guide covers the
compositions that answer the less obvious system-design questions: what happens
at a limit, where amplification comes from, how traffic meets replicas, and
whether a proposal fixed a bottleneck or merely moved it.

Each technique includes the smallest useful model and the mistake it prevents.
The syntax itself is covered by [Designing a system](./modelling.md) and
[The expression language](./language.md).

## Demand and routing

### Clamp capacity; condition subpopulations

Clamping and conditioning answer different questions. A capacity serves demand
up to its limit, so use `min`:

```squiggle
offered = 700 to 1400
capacity = 1000
served = min(offered, capacity)
served
```

Every draw above `1000` becomes exactly `1000`. The resulting distribution has
an atom at the limit whose mass is the probability that the capacity saturated:

$$P(\text{served} = c) = P(\text{offered} \geq c)$$

Conditioning selects a subpopulation instead:

```squiggle
latency = 0.01 to 0.5
timeout = 0.2
returned_before_timeout = truncate(latency, 0, timeout)
returned_before_timeout
```

`truncate` removes draws outside the interval and renormalises those left. That
is right when asking about the latency of calls that returned before their
timeout. It is wrong when work piles up against a capacity, because it discards
the overloaded cases instead of leaving them visible at the limit.

::: warning Mistake prevented
Using `truncate(offered, 0, capacity)` makes the model look healthiest precisely
when demand exceeds capacity. Use `min` for saturation and `truncate` only when
the question is explicitly conditional.
:::

### Charge retries as demand

A retry policy multiplies downstream demand by the expected attempts, not by the
eventual success probability:

$$A(p,n) = \frac{1-(1-p)^n}{p}, \qquad
P(\text{success}) = 1-(1-p)^n$$

The shipped behaviour reads $p$ from the response coming back from the
dependency:

```yaml
mutators:
  - type: retry
    properties:
      attempts: '3'
```

For a three-attempt policy the same setting has very different costs as the
dependency degrades:

| Per-attempt success | Expected attempts | Eventual success |
| ---: | ---: | ---: |
| 99% | 1.01 | 99.9999% |
| 50% | 1.75 | 87.5% |
| 20% | 2.44 | 48.8% |

At 20% success, a dependency already failing four calls in five is immediately
asked to serve 2.44 times the original traffic. That closes the retry-storm loop:
failure creates demand, and demand creates more failure.

::: warning Mistake prevented
Multiplying reliability without multiplying rate models the benefit of retries
and omits their cost. Put the `retry` behaviour on the relationship so both move
together.
:::

### Write behaviour order deliberately

Behaviours compose in declaration order on the request path and in reverse order
on the response path. With 200 operations per second, a limit of 100, and three
attempts which are all spent, these lists differ threefold.

The first caps the original demand, then retries what passed the cap:

```yaml
mutators:
  - type: load-shed
    properties:
      limit: '100'
  - type: retry
    properties:
      attempts: '3'
```

The dependency receives up to 300 operations per second. Reversing the list
amplifies first and caps the result:

```yaml
mutators:
  - type: retry
    properties:
      attempts: '3'
  - type: load-shed
    properties:
      limit: '100'
```

The dependency receives at most 100 operations per second. Neither ordering is
universally correct: the first limits admitted user calls, while the second
protects the dependency from attempts. The model must match where the real
policies run.

::: warning Mistake prevented
A list of individually reasonable policies is not order-independent. Check the
rate after every behaviour whenever one amplifies and another caps it.
:::

### Route migrations with complementary flags

A migration is a dial, not a switch. Name the new path's share once and send its
complement down the old path:

```yaml
# _system.yaml
scratchpad:
  - name: new_share
    expression: '0.2'
    unit: share
    summary: Share of requests routed to the replacement.
```

```yaml
# components/users.yaml
outgoing:
  - to: replacement
    mutators:
      - type: feature-flag
        properties:
          exposure: new_share
  - to: legacy
    mutators:
      - type: feature-flag
        properties:
          exposure: 1 - new_share
```

At `new_share = 0.2`, the replacement receives 20% and the legacy service still
receives 80%. Solve several shares because either side can become the binding
path while the dial turns.

::: warning Mistake prevented
Sizing only the destination ignores the load the source continues to carry. A
routed migration must contain both paths and preserve `new_share + old_share = 1`.
:::

### Hold a feature dark before launch

Keep a component in the design behind a flag whose baseline is zero, then expose
it with interventions:

```yaml
scratchpad:
  - name: recommender_exposure
    expression: '0'
    unit: share
    summary: Share of requests which invoke recommendations.

interventions:
  - id: canary
    name: Canary recommendations
    overrides:
      - name: recommender_exposure
        expression: '0.05'
  - id: launch
    name: Launch recommendations
    overrides:
      - name: recommender_exposure
        expression: '1'
```

```yaml
outgoing:
  - to: recommender
    mutators:
      - type: feature-flag
        properties:
          exposure: recommender_exposure
```

The baseline proves the existing design with the path dark. `canary` prices the
first 5%, and `launch` reveals any constraint full exposure would introduce:

```sh
optimist compare ./design canary launch
```

::: warning Mistake prevented
Adding the component only after launch makes its capacity requirement invisible
until traffic reaches it. Model the path at zero exposure and compare the rollout
before shipping it.
:::

### Put schedules in shared quantities

Every shared expression can read `t`; schedules are not special to intervention
overrides. This flag remains dark for five seconds and then ramps over ten:

```yaml
scratchpad:
  - name: rollout_share
    expression: 'min(max((t - 5) / 10, 0), 1)'
    unit: share
    summary: Replacement traffic, ramped from t = 5 to t = 15.
```

Any component or behaviour may refer to `rollout_share`. The same pattern models
a traffic ramp, a piecewise diurnal load curve, or a staged dependency
withdrawal:

```sh
optimist solve ./design --horizon 20 --step 1
```

A constant is only the simplest time function. Use an intervention when asking
how a proposal differs from the baseline; use `t` in the baseline when the
system itself changes over the run.

::: warning Mistake prevented
Encoding a rollout as several unrelated static designs loses the load carried by
intermediate stages and makes their results difficult to compare.
:::

## Capacity boundaries

### Read binding probability beside mean utilisation

Mean utilisation answers what an average draw does. It does not answer how often
the limit is crossed. For example, a caller with this demand against a pool with
capacity of 1,000 operations per second averages only 60% utilisation:

```yaml
# client
properties:
  request_rate: 'lognormal({mean: 600, stdev: 500})'

# compute pool
properties:
  service_time: '0.001'
  parallelism: '1'
```

The tail still exceeds capacity in roughly 14% of draws. Ask Optimist for both:

```sh
optimist bottlenecks ./design --samples 20000
```

`MEAN` describes average load, `P90` shows a tail reading, and `BINDS` is the
share of draws where demand met or exceeded the limit. Ranking leads on `BINDS`
because a variable constraint at 60% mean can fail more often than a stable one
at 80%.

::: warning Mistake prevented
A mean below one is not headroom in every plausible world. Do not approve a
capacity plan without reading its probability of binding.
:::

### Choose sharded or mirrored scale units

Scale-unit distribution decides whether each replica receives a share of demand
or the whole of it:

```yaml
scale_units:
  - id: api-fleet
    name: API replicas
    replicas: '3'
    distribution: sharded
    members: [api]
```

With 120 operations per second, each API replica receives 40. Change only the
distribution:

```yaml
scale_units:
  - id: replicated-store
    name: Replicated stores
    replicas: '3'
    distribution: mirrored
    members: [store]
```

Each store receives all 120 operations per second. The deployment performs 360
replica-operations in total, but every per-replica constraint is still evaluated
against 120.

| Distribution | Per-replica demand | Typical use |
| --- | ---: | --- |
| `sharded` | total / replicas | Load-balanced workers, partitioned data |
| `mirrored` | total | Replicated writes, quorum members |

::: warning Mistake prevented
Marking mirrored work as sharded sizes every replica for a fraction of the load
it will actually receive. Replication can multiply cost without adding throughput.
:::

### Preserve dimensions in formulas

Two formulas which sound right in prose are dimensionally wrong:

| Expression | Actual unit | Required correction |
| --- | --- | --- |
| `cores / service_time` | `s^-1` | Represent concurrent operation slots as `op`. |
| `request_rate * payload_bytes` | `B*op/s` | Represent payload as bytes per operation, `B/op`. |

A component type should state the quantities the arithmetic needs:

```yaml
properties:
  parallelism:
    unit: op
    summary: Operations which can be in flight at once.
  service_time:
    unit: s
    summary: Time one operation occupies a slot.
  record_size:
    unit: B/op
    summary: Bytes transferred by one operation.

channels:
  capacity:
    unit: op/s
    expression: Little.rate(parallelism, service_time)
  transfer:
    unit: B/s
    expression: arriving * record_size
```

For capacity, the useful quantity is not a bare core count but the number of
in-flight operations those cores sustain. For bandwidth, payload is not merely
bytes but bytes per operation.

::: warning Mistake prevented
Attaching the desired unit to an incorrectly dimensioned result does not repair
the model. Name the missing operational quantity so unit algebra can check the
formula.
:::

### Let record size select the storage limit

A datastore has separate operation and transfer limits. Hold those limits and
the request rate fixed:

```yaml
properties:
  operation_limit: '1000'
  transfer_limit: '1e6'
  volume_limit: '1e15'
  retention: '3600'
  record_size: record_size
```

At 900 operations per second, record size alone changes the answer:

| Record size | Operation utilisation | Transfer utilisation | First limit |
| ---: | ---: | ---: | --- |
| 100 B/op | 90% | 9% | operations |
| 5,000 B/op | 90% | 450% | transfer |

The device did not change. Small records spend its operation budget; large
records spend its byte budget.

::: warning Mistake prevented
Reducing a store to one throughput number assumes record shape cannot change.
Keep operation rate and byte rate as independent constraints.
:::

### Declare each physical limit once

A constraint should pair offered demand with a physical limit. These three
constraints restate the same boundary:

```yaml
channels:
  served:
    unit: op/s
    expression: min(arriving, capacity)
  utilisation:
    unit: '1'
    expression: arriving / capacity

constraints:
  capacity:
    demand: arriving
    limit: capacity
  utilisation:
    demand: utilisation
    limit: '1'
  served:
    demand: served
    limit: capacity
```

Keep only the first. `utilisation >= 1` is algebraically identical to
`arriving >= capacity`, so it adds a duplicate to the ranking. `served` is worse:
clamping forces it to stop at one, hiding how far offered demand exceeded the
limit while still reporting a binding constraint.

Derived queue length, accepted rate, and saturated throughput are useful
channels. They become constraints only when they consume a distinct physical
resource such as buffer depth.

::: warning Mistake prevented
Repeated versions of one limit crowd the ranking and can make a saturated result
look like independent evidence. Constrain the unsaturated offered quantity once.
:::

## Reading results

### Read `introduced` with `relieved`

The worst baseline constraint often stops being worst after a useful change.
That promotion is expected; the question is whether the promoted constraint now
binds:

```sh
optimist compare ./design larger-store
```

```text
COMPONENT  CONSTRAINT  UTILISATION  BINDS       EFFECT
store      volume      1.20 -> 0.60  80% -> 0%   relieved
link       bandwidth   0.80 -> 1.30   0% -> 35%  introduced
api        capacity    0.70 -> 0.82   0% -> 0%   loaded
```

`relieved` means a constraint bound before and never binds after. `introduced`
means it never bound before and binds in some proposed draws. `eased` and
`loaded` describe movement without crossing that boundary.

The example rearranged the bottleneck from storage to the link. A real fix has
the intended `relieved` entries, no unacceptable `introduced` entries, and
enough headroom in what remains.

::: warning Mistake prevented
Looking only at the original bottleneck makes any larger limit look successful.
Always inspect what the proposal introduced and what remains binding.
:::

### Compare settled values relatively

Optimist relaxes feedback toward a fixed point. A draw is settled when every
movement is within the relative tolerance, $10^{-6}$ by default:

$$\delta = \frac{|x_{next} - x_{previous}|}
{\max(|x_{next}|, |x_{previous}|, 1)}$$

The floor at one prevents a value approaching zero from being asked for
ever-finer absolute agreement. It also means every quantity read after the first
step can carry a small residual. At a scale of one billion, a final difference
around one thousand is still one part per million; near zero, the absolute
tolerance is one millionth.

Use the same comparison in checks derived from solved output:

```python
def close(actual, expected, tolerance=1e-6):
    scale = max(abs(actual), abs(expected), 1.0)
    return abs(actual - expected) <= tolerance * scale
```

Exact equality remains appropriate for authored constants and identities the
runtime preserves exactly. It is not appropriate for values reached by iterative
relaxation.

::: warning Mistake prevented
Comparing settled floating-point values absolutely makes large quantities look
wrong and tiny quantities look arbitrarily precise. Compare them at their own
scale.
:::
