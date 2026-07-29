# Solving and bottlenecks

Solving a design means finding the quantities that flow through it. Ranking it
means turning those quantities into the one answer worth having: which resource
this design is closest to exhausting.

## Why the solver iterates

Channels can be arranged in dependency order only when a model is acyclic, and
the models worth building rarely are. Utilisation sets queueing delay, delay sets
how long each request occupies a worker, occupancy sets utilisation again. That
loop has no first term to evaluate.

The solver therefore relaxes toward a fixed point. It evaluates every component
against the current estimate of its inputs, blends the result part of the way
toward what it computed, and repeats until nothing moves. Where a model happens
to be acyclic this converges immediately, so ordering is never needed as a
special case.

Blending rather than jumping is deliberate. A cancelling timeout and the load it
relieves form an oscillator: cancelling lowers utilisation, which lowers latency,
which stops the cancelling, which raises the load again. Moving the whole way
each pass overshoots and the iterate cycles instead of settling.

## Not converging is a result

An iteration that never settles is reported rather than hidden behind a last
iterate:

```text
Did not settle after 1500 passes; largest movement 3.412e-2.
A loop whose gain exceeds one has no steady state to find.
```

A loop with gain above one has no steady state, and saying so is more useful than
returning whichever values the cap happened to stop at. Because relaxation runs
per draw, the share of draws still moving distinguishes a wholly unstable design
from one unstable only in its tail.

The commonest cause is unbounded amplification: a retry policy against a
dependency that cannot serve the amplified demand, or a component type on a
response leg that publishes `rate` and so feeds demand back into its own caller.

## Settling on several states is also a result

Not settling and having several answers are different things, and only the first
is a failure. Past a fold a draw can sit on a branch steeper than the damped step
can follow and swap between two values indefinitely, while the ensemble it
belongs to is perfectly still — the same values in the same proportions on every
pass, only trading places.

So where the per-draw test gives up, the solver asks a second question: has the
*distribution* stopped moving? It compares order statistics rather than draws,
which removes the assignment of values to draws and leaves the empirical
distribution, invariant under any permutation of the branches. A stationary
mixture then reads as no movement at all, and is reported as settled on several
states with the quantity and the count named.

That is a finding rather than a fault: the figures are what the design does. What
it warns about is the mean, which is taken across the branches and describes
none of them.

## Steady state and transient

The same equations either way. What differs is whether the backlog on each wire
is asked to balance or asked to move.

**Steady** is the default. One algebraic solve, using the closed form for a
bounded queue, so the answer arrives immediately. It says where the design comes
to rest, which is the question being asked nearly all of the time, and it is what
a constraint should be read against.

It has no memory. Where a design has more than one resting state, this reports
the one reachable from nothing, so a surge that would have tipped it over and
left it there appears to be survived.

**Transient** advances the backlog through time, one step at a time.

```sh
optimist solve examples/metastable --transient --horizon 250 --step 0.1
```

The queue on each wire fills and drains at a finite rate, which is what gives a
design memory. A buffer filled by a surge has to be emptied afterwards, and if
work arrives faster than it drains the design stays where the surge left it.
Hysteresis, recovery time, and whether an incident ends when its cause does are
only visible here.

The cost is the step. Integration is faithful only while a step is short against
the time a queue takes to drain, so shorten `--step` and lengthen `--horizon`
together. A horizon that reads comfortably in seconds may need thousands of
steps.

## Time

`--horizon` is the number of steps and `--step` their length in seconds. Time is
visible to expressions as `t`, so a scratchpad quantity or an intervention
override can depend on it:

```yaml
overrides:
  - name: peak_rate
    expression: 'if t < 10 then 4000 else 900'
```

That is a ten-second surge. Under steady solving each step is solved from rest
and the surge leaves no trace; under transient solving the backlog it created has
to drain, and whether it does is the whole question.

`prev.<channel>` gives a component its own values from the previous step, which
is how a component type carries state of its own. `dt` is the step length, so an
accumulation is written as an accumulation — and `steady` says which question is
being asked, so the same channel can report where the design rests without that
answer depending on how long a step the solver took:

```yaml
backlog:
  expression: >
    if steady
      then Queue.boundedLength(load, capacity)
      else max([prev.backlog + (arrivals - departures) * dt, 0])
```

## More than one fixed point

The solver starts from a single set of values, so where a loop admits more than
one fixed point it reports the one reachable from rest — the lower, uncongested
branch of a bistable system. The congested branch exists and is not searched for.

A wide converged distribution is the signal that a design is operating near the
fold between the two. Some draws settle healthy and some settle collapsed, and
the result is a genuine mixture rather than a broad unimodal spread. This is why
the workbench draws a density estimate and calls out multiple modes rather than
leaving them to a mean and a percentile pair.

The `metastable` example is built to demonstrate exactly this. See
[the examples](../examples/README.md).

## Ranking constraints

Every constraint pairs a demand with the limit it consumes. Utilisation is their
ratio, taken per draw, so the answer is a distribution rather than a figure:

```sh
optimist bottlenecks examples/checkout
```

```text
╭─ Constraints ────────────────────────────────────────────────────────────────╮
│ COMPONENT  CONSTRAINT        LOAD            MEAN     P90  BINDS    HEADROOM │
│ ─────────  ────────────────  ────────────  ──────  ──────  ─────  ────────── │
│ orders     volume            ████████████    7.01    9.56   100%   -3.004e12 │
│ api        capacity          ████████████    2.96    4.92    87%  -1063.2349 │
│ browsers   success_objecti…  ████████████   55.63     110    86%     -0.2731 │
│ browsers   latency_objecti…  ██████░░░░░░  0.4597  0.7931     3%      0.4053 │
│ orders     operations        █░░░░░░░░░░░   0.066    0.09     0%   4669.9294 │
╰──────────────────────────────────────────────────────────────────────────────╯

╭─ orders.volume runs out first ───────────────────────────────────────────────╮
│ It is carrying 7.01× what its limit allows on average and binds in 100% of   │
│ draws. Resident bytes against usable capacity. Unlike the rate limits this   │
│ one fills gradually and then fails abruptly, so headroom here is measured in │
│ time rather than in load.                                                    │
╰──────────────────────────────────────────────────────────────────────────────╯
```

| Column | Meaning |
| --- | --- |
| `LOAD` | Mean utilisation drawn as a bar, filled completely at or beyond the limit. |
| `MEAN` | Mean of demand over limit. |
| `P90` | Utilisation at the ninetieth percentile of draws. |
| `BINDS` | Share of draws in which demand met or exceeded the limit. |
| `HEADROOM` | Mean limit less mean demand, in the constraint's own units. |
| `REPLICAS` | Replicas of the owning component across every enclosing scale unit. Shown only where a design has any; the other figures describe **one** replica. |

Constraints are ordered by how likely they are to bind, and by utilisation where
that ties:

$$P(\text{bind}) = \frac{1}{n}\sum_{i=1}^{n} \mathbb{1}\{d_i \geq l_i\}$$

Ranking by probability rather than by mean utilisation puts the constraint most
exposed to a bad draw at the top, which is the one worth spending on. Two
constraints at the same average load are not equally urgent if one of them is far
more variable.

Add `--binding` to drop everything with headroom in every draw.

The engine attaches no meaning to any constraint's name. A limit called `iops`
and one called `concurrency` are ranked by identical arithmetic, which is what
lets a new component type introduce a resource nobody anticipated and still have
it reported.

### Objectives are constraints too

A `client` compares the latency and success arriving back at it against the
targets it declares. Because responses propagate back along every relationship,
those figures already include every hop, retry, timeout, and fan-out along the
way. That turns "does this design meet its objective" into a ranked constraint
rather than a figure an engineer has to assemble by hand.

## Weighing a change

```sh
optimist compare examples/checkout warm-cache
```

```text
╭─ warm-cache ────────────────────────────────────────────────────────────────╮
│ COMPONENT  CONSTRAINT               UTILISATION      BINDS  EFFECT          │
│ ─────────  ─────────────────  ─────────────────  ─────────  ─────────       │
│ orders     volume                 7.01 → 0.6433  100% → 0%  relieved        │
│ orders     operations            0.066 → 0.0061    0% → 0%  eased           │
│ browsers   latency_objective    0.4597 → 0.4955    3% → 8%  loaded          │
│ api        capacity                 2.96 → 3.19  87% → 87%  loaded          │
│ browsers   success_objective      55.63 → 74.73  86% → 86%  loaded          │
╰─────────────────────────────────────────────────────────────────────────────╯

╭─ warm-cache relieves what it was aimed at ──────────────────────────────────╮
│ It stops 1 constraint binding and starts none: orders.volume. 3 constraints │
│ are still binding afterwards: browsers.latency_objective, api.capacity,     │
│ browsers.success_objective.                                                 │
╰─────────────────────────────────────────────────────────────────────────────╯
```

`compare` solves the design twice — once as it stands, once with the
intervention's rebindings applied — using the same seed and the same draws, and
reports the movement of every constraint. Name several interventions at once and
they become as comparable with each other as each is with the baseline.

| Effect | Meaning |
| --- | --- |
| `relieved` | Bound in some draws before, in none after. |
| `introduced` | Bound in no draws before, in some after. |
| `eased` | Utilisation fell. |
| `loaded` | Utilisation rose. |
| `unchanged` | Utilisation did not move. |

The note beneath the table says which of three things the change did: relieved
what it was aimed at, moved the bottleneck somewhere else, or changed nothing
that binds. It also names whatever is still binding afterwards.

That is the point of comparing rather than re-solving. Relieving the constraint
everybody was watching usually promotes the next one, and a proposal is only
worth funding if the design is better after the promotion.

## Controls

| Flag | Default | Effect |
| --- | --- | --- |
| `--seed` | `0` | Root of the deterministic random stream. |
| `--samples` | `1000` | Draws carried through every uncertain quantity. |
| `--horizon` | `1` | Number of steps to advance. |
| `--step` | `1.0` | Length of one step, in seconds. |
| `--transient` | off | Advance queues through time instead of solving for balance. |
| `--intervention` | none | Apply an intervention before solving or ranking. Short form `-i`. |
| `--component` | all | Report or rank only one component. Short form `-c`. |

The same controls are query parameters on the HTTP analysis endpoint, where
`samples` is clamped to 64–20,000 and `horizon` to 1–500.

Relaxation itself is not exposed on the command line. The iteration cap is 1,500
passes, the tolerance for "settled" is a relative movement of $10^{-6}$, and the
blend is a fraction of the way toward each computed value. Feedback converges
more slowly than a feed-forward chain: a retry policy against a saturated
dependency has a loop gain just under one, so the iterate approaches its fixed
point steadily but without hurry. A pass is cheap, and a loop that genuinely has
no fixed point diverges fast enough to be obvious long before the cap.

## Reading the solved quantities

```sh
optimist solve examples/checkout --component api
```

Alongside a component's own channels, the report includes what it read from its
ports:

- `in.<port>.<signal>` — demand arriving on an inbound port, aggregated across
  callers and after every behaviour on those relationships.
- `out.<port>.<signal>` — responses returning on an outbound port, which is what
  explains the component's own latency and failures.

Those two are usually where a surprising answer is explained. A pool that looks
overloaded with no change in demand is normally reading a `latency` on
`out.<port>` that has risen, because a synchronous worker cannot be reclaimed
while it waits and a slow dependency consumes capacity just as surely as slow
local work does.
