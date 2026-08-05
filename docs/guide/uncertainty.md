# Uncertainty

Every quantity in a design is Squiggle source. A service time nobody has measured
precisely stays a distribution the whole way through the solve, and the spread of
the answer reports exactly how much of that uncertainty has crossed into
congestion.

Every field that takes an expression previews what it evaluates to while you are
typing into it, so the spread being authored is visible before the design is
solved.

![A shared quantity being edited, with a flyout showing the density of the lognormal it evaluates to and its p10, median, and p90.](/screenshots/quantities.png)

## Squiggle in one page

Optimist embeds its own Squiggle-compatible runtime. It owns its syntax tree,
parser, values, evaluator, and statistical operations, and does not depend on the
rest of the tool, so the same expressions behave the same in a component
property, a scratchpad entry, an intervention override, and a behaviour's
settings.

```squiggle
lognormal(-4.6, 0.35)                    // a distribution
0.02                                     // a certainty
mixture(0.02, 0.4, [0.9, 0.1])           // a bimodal service time
peak_rate * 1.4                          // arithmetic over a shared quantity
if t >= 5 && t < 15 then 3600 else 900   // a function of the timeline
```

Blocks and named bindings work, which is how a long expression stays readable:

```squiggle
{
  base = lognormal(-4.6, 0.35)
  overhead = 0.004
  base + overhead
}
```

Distributions are available symbolically under `Sym.` and as sampled values under
`Dist.`. The symbolic families are `Sym.normal`, `Sym.lognormal`, `Sym.uniform`,
`Sym.beta`, `Sym.cauchy`, `Sym.gamma`, `Sym.logistic`, `Sym.exponential`,
`Sym.bernoulli`, and `Sym.triangular`; `poisson`, `binomial`, and `pointMass` are
globals only. Symbolic families are preserved where the arithmetic allows and
fall back to seeded empirical draws where it does not.

The usual collection, dictionary, list, string, date, and duration namespaces are
present. `optimist catalogue <design> --output json` includes a `builtins` list
of every name an expression may call, which is the same list the workbench uses
for completion.

::: tip The whole language
This page is about *where* uncertainty belongs. For the syntax, the operators,
the precedence table, every builtin, and the names available inside a design, see
[the expression language](./language.md). For which distribution to reach for and
what to put in it, see [choosing distributions](./distributions.md).
:::

## Sample sets and alignment

A quantity is carried as a fixed number of aligned draws. Draw $i$ of one channel
is combined with draw $i$ of every other, so each draw index carries a complete
deterministic system that settles on its own fixed point independently of the
others.

This matters more than it first appears. Uncertainty is not smeared through the
loop and then summarised at the end: where demand is uncertain enough that some
draws saturate and others do not, the converged result is a genuine mixture, and
its spread is a statement about how much of the distribution has crossed into
congestion.

It also means a summary can mislead. A mean and a central interval describe a
bimodal mixture exactly as they would describe one broad unimodal spread, which
is why the workbench draws a density estimate rather than a summary and says so
in words when it finds more than one mode.

Control the draw count with `--samples`. A thousand is the default and is enough
for a mean; ten thousand is more honest about a ninetieth percentile. The HTTP
API clamps the request to between 64 and 20,000 draws, because draw count is the
one control that costs the server rather than the caller.

## Determinism

`--seed` is the root of the random stream, and defaults to zero. The same
directory solved with the same seed, draw count, horizon, and step produces the
same numbers on any machine.

Two consequences are worth stating. First, a design that has been through
persistence is stored in a canonical order, so a model assembled by hand and one
assembled by the workbench solve identically. Second, a comparison between a
baseline and an intervention uses the same stream for both, so a difference in
the result is a difference in the design rather than a difference in the draws.

## Units

Every property, channel, transform, and signal carries a unit annotation such as
`op/s`, `B/op`, `s`, or `1` for a plain dimensionless number. Annotations are
parsed and validated when a definition loads, so a manifest with a malformed unit
is rejected rather than discovered later.

A proportion of a whole says so with `share`, which carries no dimension of its
own — multiplying a rate by a share still checks as a rate — and marks the figure
as one to read as a percentage. The distinction is what separates a success of
`0.97` from a fan-out of `3`: both are pure numbers, and only one of them is
ninety-seven percent of anything. `ratio`, `fraction`, `proportion`, and
`probability` are the same annotation under other names.

Units document the intent of a quantity and are what allow a report to label a
figure it derived. They are not currently used to reject a component that
supplies a property in the wrong dimension, so read the `summary` beside a
property before supplying it.

## Queueing and reliability

The runtime ships domain namespaces so that a manifest states a law rather than
reimplementing it. The equations, assumptions, and limitations of each are in
[laws and models](../reference/laws.md); what follows is the working summary.

### `Little`

Little's Law relates the work resident in a system, the rate through it, and how
long each unit stays:

$$L = \lambda W$$

| Function | Returns |
| --- | --- |
| `Little.occupancy(rate, residence)` | Work in flight, $L$. |
| `Little.residence(occupancy, rate)` | Time each unit stays, $W$. |
| `Little.rate(occupancy, residence)` | Throughput, $\lambda$. |

The shipped `compute` type sizes itself with `Little.rate(servers, hold_time)`:
concurrent slots divided by the time each request holds one.

### `Queue`

| Function | Returns |
| --- | --- |
| `Queue.utilisation(demand, capacity)` | $\rho = \lambda / \mu$, also spelled `Queue.utilization`. |
| `Queue.mm1Wait(service, utilisation)` | Mean waiting time in an M/M/1 queue. |
| `Queue.mmcWait(service, servers, utilisation)` | Mean waiting time in an M/M/c queue, via Erlang C. |
| `Queue.erlangB(servers, offered)` | Blocking probability with no waiting room. |
| `Queue.erlangC(servers, offered)` | Probability an arrival has to wait. |
| `Queue.boundedLength(utilisation, capacity)` | Mean number waiting in an M/M/1/K queue. |
| `Queue.boundedBlocking(utilisation, capacity)` | Probability an arrival is turned away by a full M/M/1/K queue. |

Offered load in erlangs is $a = \lambda / \mu = \lambda S$ for mean service time
$S$, and utilisation is $\rho = a / c$. The Erlang recursions are evaluated in a
form that avoids computing factorials directly, so a large server count does not
overflow.

### `Reliability`

| Function | Returns |
| --- | --- |
| `Reliability.retrySuccess(attempt, attempts)` | $1 - (1-p)^n$, the chance a call eventually succeeds. |
| `Reliability.retryAttempts(attempt, attempts)` | $\frac{1 - (1-p)^n}{p}$, the expected attempts made, and therefore the amplification applied downstream. |
| `Reliability.serialSuccess(step, steps)` | $p^k$, for a call that must complete $k$ independent steps. |
| `Reliability.deadlineSuccess(steps, service, deadline)` | $P(k, D/S) = \gamma(k, D/S)/\Gamma(k)$, the chance an Erlang-distributed request finishes in time. |
| `Reliability.quorumSuccess(node, nodes, required)` | $I_p(r,\, n-r+1)$, the chance at least $r$ of $n$ independent nodes succeed. |
| `Reliability.quorumLatency(node, nodes, required)` | $L\,(H_n - H_{n-r})$, the mean time until the $r$th of $n$ exponential replies arrives. |

`retryAttempts` is the term that turns a partial outage into a retry storm: as
$p$ falls it grows toward the full budget, so every caller multiplies its load on
the dependency that is already failing. It is why the shipped `retry` behaviour
reads the success rate coming back rather than taking a constant.

Independence is the load-bearing assumption in all of these and it is optimistic.
Attempts against a saturated dependency fail together, so correlated failure has
to be expressed through shared upstream uncertainty — a single scratchpad
quantity that several components read — rather than by relying on these formulas.

`deadlineSuccess` assumes exponential steps, which is the maximum-variability
choice for a given mean, so it is a conservative estimate of meeting a deadline.

The two quorum laws are the only place in the vocabulary where adding a
dependency makes a design *better*. Needing a majority rather than all of them
inverts both readings at once: reliability rises with the group instead of
falling, and the wait shrinks, because the slowest and the failed are exactly the
nodes a majority leaves behind. Three nodes at $p = 0.99$ reach $0.9997$ together
where needing all three gives $0.970$, and answer in $0.83L$ where waiting for
all three takes $1.83L$. Neither buys throughput: every node still receives every
request.

### `Slo`

`Slo.errorBudget` and `Slo.burnRate` express an objective as the failures it
permits over a window, and observed failure as a multiple of that allowance.
A burn rate of one exhausts the budget exactly at the end of the window.

## Where uncertainty belongs

Put the spread on the quantity that is actually uncertain, and put it there once.

- **A measured quantity with dispersion** — a service time, a payload size —
  belongs on the property, as a distribution.
- **A quantity several components share** — peak demand, a record size, a
  peak-to-mean ratio — belongs in the scratchpad. Two components reading one
  entry see the *same* draw, which is what preserves the common cause between
  them. Two components each authoring their own `lognormal` do not, and the
  design will understate its correlated risk.
- **A quantity you intend to vary** belongs in the scratchpad too, because an
  intervention can only rebind something that has a name.

A hit ratio measured on today's traffic is a poor guide to tomorrow's, and a
design that depends on a high one should be checked against a low one. That check
is a distribution on `cache_hits`, not a second design.

## Reading the result

A solved quantity is charted across the horizon with its distribution shaded
around it, and stopping on a step draws the spread behind that point.

![The simulation view charting success rate and response time, with the distribution across draws shaded around each line.](/screenshots/simulation.png)

The CLI reports the same figures as a mean and a central eighty percent interval:

```text
api  capacity  685.1550 [450.9287 .. 947.7374]
```

A constraint is reported with more than that, because the mean is the least
useful part:

```text
COMPONENT  CONSTRAINT  UTILISATION  P90    BINDS  REPLICAS  HEADROOM
api        capacity    2.960        4.916  87%    1         -1063.2349
```

`BINDS` is the share of draws in which demand met or exceeded the limit:

$$P(\text{bind}) = \frac{1}{n}\sum_{i=1}^{n} \mathbb{1}\{d_i \geq l_i\}$$

A constraint at 60% mean utilisation may still exceed its limit in a fifth of
draws. That is the figure worth acting on, and it is why the ranking uses it.
See [solving and bottlenecks](./analysis.md) for the rest of the arithmetic.
