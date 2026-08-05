# The expression language

Every quantity in an Optimist design is a Squiggle expression. A component
property, a channel, a scratchpad entry, an intervention override, a behaviour's
settings — all of them are source, and all of them are evaluated by the same
embedded runtime. That is why `0.02` and `0.02 * lognormal(0, 0.15)` are
interchangeable in the same field: one is a certainty, the other is a belief, and
the solver carries both the same way.

The workbench previews what each field evaluates to while you type into it, so
the value you are authoring is visible before the design is solved. If an
expression is wrong, you find out in the field rather than in the result.

::: tip Three pages, three jobs
This page is the syntax and the vocabulary. [Choosing a distribution](./distributions.md)
is the modelling judgement about which family fits which quantity.
[Uncertainty](./uncertainty.md) is how draws are carried, aligned, and read back.
:::

## Values

| Value | Written as | Notes |
| --- | --- | --- |
| Number | `42`, `0.02`, `.001`, `1.`, `0.1e-3` | Always a 64-bit float |
| Boolean | `true`, `false` | |
| String | `'ok'`, `"ok"` | Both quotes; `\n \r \t \\ \' \" \uABCD` escape |
| Distribution | `lognormal(0, 0.3)`, `1 to 4` | Symbolic where it can be, sampled where it cannot |
| List | `[1, 2, 3]`, `[3, 4,]` | Heterogeneous is allowed; trailing commas are fine |
| Dictionary | `{a: 1, b: 2}`, `{a}` | Keys are strings; `{a}` is shorthand for `{a: a}` |
| Function | `f(x) = x * 2`, `{\|x\| x * 2}` | First class; pass one to `List.map` |
| Date | `Date.make('2020-01-01')` | |
| Duration | `90minutes`, `1days` | A distinct type, not a number |

### Magnitude and duration suffixes

A bare number may carry a suffix. Magnitudes scale it; `minutes`, `hours`,
`days` and `years` (or `year`) turn it into a `Duration`.

| `n` | `m` | `%` | `k` | `M` | `B`, `G` | `T` | `P` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| $10^{-9}$ | $10^{-3}$ | $10^{-2}$ | $10^{3}$ | $10^{6}$ | $10^{9}$ | $10^{12}$ | $10^{15}$ |

```squiggle
100%              // 1
2k + 3            // 2003
250m              // 0.25
Duration.toHours(90minutes)   // 1.5
```

::: warning `m` is milli, not metres
`50m` is `0.05`. If you want to annotate a quantity as metres, that is the unit
annotation `::`, not a suffix — see [units](./uncertainty.md#units).
:::

## Syntax

Comments are `//` to the end of the line, or `/* ... */` across lines. Statements
are separated by a newline **or** a semicolon, so these are the same program:

```squiggle
base = 0.01
spread = 0.15
base * lognormal(0, spread)

// or, on one line
base = 0.01; spread = 0.15; base * lognormal(0, spread)
```

The value of a program is its final expression, and that expression must come
last. Putting a binding after it fails with `the module result must be its final
expression`.

### Blocks

A block is a `{ ... }` with statements inside it, and its value is the last
expression in it. This is how a long quantity stays readable:

```squiggle
{
  hit = 0.85
  fast = 0.002 * lognormal(0, 0.2)
  slow = 0.05 * lognormal(0, 0.4)
  mixture(fast, slow, [hit, 1 - hit])
}
```

Bindings inside a block are local to it, and nesting works:
`x = { y = { z = 5; z * 2 }; y + 3 }` is `13`.

### Functions and lambdas

A named function is a binding with a parameter list. A lambda is a brace with
pipes: `{|x, y| ... }`, or `{|| ... }` when it takes nothing.

```squiggle
capacity(servers, hold) = servers / hold
capacity(30, 0.05)                       // 600

List.map([10, 20, 30], {|x, i| x + i + 1})   // [11, 22, 33]
```

Functions capture the environment where they were defined, not where they are
called:

```squiggle
x = 5; f(y) = x * y; x = 6; f(2)   // 10
```

Parameters may declare a domain, which is checked at the call:

```squiggle
share(p: Number.rangeDomain(0, 1)) = p
share(2)   // error: outside its declared domain
```

Bindings, parameters and return values may carry a `::` unit annotation, which is
checked by the linter rather than rescaled at runtime:

```squiggle
distance :: m = 10
time :: s = 2
speed :: m/s = distance / time

throughput(servers :: op, hold :: s) :: op/s = servers / hold
```

### The pipe

`->` sends the value on its left in as the first argument of the call on its
right. It is the readable way to chain summaries.

```squiggle
normal(0, 1) -> mean
f(x, y) = x + y; 1 -> f(2)      // 3
6 -> {|x, y| x / y}(2)          // 3
0.01 * lognormal(0, 0.4) -> quantile(0.95)
```

### Conditionals

Both forms exist and mean the same thing. Reach for `if/then/else` when the
branches are long, and `? :` when they are short.

```squiggle
if t >= 5 && t < 15 then base_demand * surge_factor else base_demand
load > 0.8 ? degraded_latency : nominal_latency
```

The condition must be a Boolean. `if 1 then 2 else 3` is a lint error, not a
truthiness coercion.

### Indexing

`xs[i]` indexes a list by a whole number; `d.key` and `d['key']` read a
dictionary.

```squiggle
([0, 1, 2])[1]      // 1
r = {a: 1}; r.a     // 1
a = 1; {a, b: a}    // {a: 1, b: 1}
```

### Precedence

Tightest binding first. `^` and `.^` are right associative; everything else in
the table is left associative.

| Level | Operators |
| --- | --- |
| 1 | `f(x)`, `xs[i]`, `d.key` |
| 2 | prefix `-`, `!` |
| 3 | `->` |
| 4 | `^`, `.^` *(right associative)* |
| 5 | `*`, `/`, `.*`, `./` |
| 6 | `+`, `-`, `.+`, `.-` |
| 7 | `to` |
| 8 | `<`, `<=`, `>`, `>=` |
| 9 | `==`, `!=` |
| 10 | `&&` |
| 11 | `\|\|` |
| 12 | `? :`, `if/then/else` |

So `2^3^2` is `512`, `1 * 2 + 3 * 4` is `14`, and `1 to 4 * 2` is `1 to 8`.

## Distributions

### `a to b`

The shorthand every capacity model reaches for. `a to b` is a **lognormal whose
5th percentile is `a` and whose 95th percentile is `b`** — a 90% credible
interval, computed with $z_{95} = 1.6449$:

$$\mu = \frac{\ln a + \ln b}{2}, \qquad \sigma = \frac{\ln b - \ln a}{2 z_{95}}$$

```squiggle
700 to 1300     // 90% sure the peak lands in here
```

::: warning `to` is lognormal only
Both bounds must be positive and ordered. `0 to 5` and `-2 to 4` both fail with
`'to' requires 0 < low < high`, and non-numeric operands fail with `'to' requires
Number operands`. There is no normal-valued fallback for non-positive bounds; if
you need one, write `Sym.normal({p5: -2, p95: 4})` explicitly.
:::

### Constructors

The full list of families lives in [Choosing a distribution](./distributions.md#what-optimist-ships).
Three of them also accept a dictionary, which is usually closer to what you
actually know:

| Form | Meaning |
| --- | --- |
| `normal({mean: m, stdev: s})` | Parameterised directly |
| `normal({p5: a, p95: b})` | 90% credible interval |
| `normal({p10: a, p90: b})` | 80% credible interval |
| `normal({p25: a, p75: b})` | 50% credible interval |
| `lognormal({mean: m, stdev: s})` | Mean and standard deviation of the **value**, not the log |
| `lognormal({p5: a, p95: b})` | Same three interval pairs; bounds must be positive |
| `beta({mean: m, stdev: s})` | Solved for $\alpha$ and $\beta$ |

```squiggle
quantile(Sym.normal({p5: -2, p95: 4}), 0.05)    // -2
quantile(Sym.lognormal({p10: 2, p90: 5}), 0.9)  // 5
```

`mixture(...)` — spelled `mx(...)` when you are in a hurry — takes components
either as arguments or as a list, with an optional list of weights:

```squiggle
mixture(0.002 * lognormal(0, 0.2), 0.05 * lognormal(0, 0.4), [0.9, 0.1])
mx(1, 2, 3)                       // equal weights
```

`pointMass(v)` is a certainty carried as a distribution. `truncate(d, lo, hi)`,
`truncateLeft(d, lo)` and `truncateRight(d, hi)` **condition** rather than clamp:
draws outside the interval are removed and the remaining mass renormalised, and
the remap is monotone so alignment with everything upstream survives. Where a
limit is a capacity that demand piles up against, use `min` and `max` instead.

### `Sym`, `Dist`, `SampleSet`, `PointSet`

| Namespace | What it gives you | When it matters |
| --- | --- | --- |
| bare globals | `normal`, `lognormal`, `beta`, … | The default; what every shipped example uses |
| `Sym.` | Symbolic families | Exact `mean`, `stdev`, `quantile` with no Monte Carlo error |
| `Dist.` | `make`, constructors, `cdf`/`pdf`/`inv`, `sample`/`sampleN`, `klDivergence`, `logScore` | Reading a distribution rather than building one |
| `SampleSet.` | `make`, `fromDist`, `fromNumber`, `fromList`, `fromFn`, `toList`, `map`, `map2`, `map3` | Forcing a quantity to draws so you can transform it per draw |
| `PointSet.` | `make`, `fromDist`, `fromNumber`, `downsample`, `support` | Working with a density rather than draws |

Symbolic families are preserved wherever the arithmetic allows and fall back to
seeded empirical draws where it does not, so the distinction rarely needs your
attention. It starts to matter when you want an exact tail quantile — take that
from a `Sym.` family rather than from a sample set.

::: details `Sym.` does not carry everything
`Sym.` registers `normal`, `lognormal`, `uniform`, `beta`, `cauchy`, `gamma`,
`logistic`, `exponential`, `bernoulli`, and `triangular`. `poisson`, `binomial`,
and `pointMass` exist as globals only. `optimist catalogue <design> --output json`
lists exactly what your build accepts, which is also what the workbench completes
against.
:::

## Summarising a distribution

| Call | Returns |
| --- | --- |
| `mean(d)` | Expected value |
| `median(d)` | 50th percentile |
| `stdev(d)`, `variance(d)` | Spread |
| `quantile(d, p)` | The value at probability `p` |
| `cdf(d, x)` | $P(X \leq x)$ |
| `pdf(d, x)` | Density at `x` |
| `inv(d, p)` | Inverse CDF; the same figure as `quantile` |
| `sample(d)` | One draw, as a number |
| `sampleN(d, n)` | A list of `n` draws |
| `min(d)`, `max(d)`, `mode(d)` | Extremes and the peak |

`sum`, `product`, `sort`, `cumsum`, `cumprod` and `diff` operate on lists, and
the `List.` and `Dict.` namespaces carry the usual collection vocabulary.

## Distribution algebra and correlation

Arithmetic on distributions is elementwise across aligned draws: draw $i$ of one
quantity is combined with draw $i$ of the other. That is what keeps a feedback
loop from inventing or destroying variance, and it is why a non-linear formula
is evaluated per draw rather than at the mean — by Jensen's inequality those are
not the same number, and queueing delay is convex enough that the difference is
the whole answer.

::: warning Each mention draws independently
Sharing is by **identity**, not by text. Two textually identical constructors are
two separate quantities:

```squiggle
// Two uncorrelated service times that happen to look alike.
a = 0.01 * lognormal(0, 0.3)
b = 0.01 * lognormal(0, 0.3)
a - b                                 // spread of a difference

// One quantity, read twice. The common cause survives.
s = 0.01 * lognormal(0, 0.3)
s - s                                 // exactly zero, every draw
```

This is the single most consequential thing to know about writing Optimist
expressions. If two components should share a risk — one peak demand, one record
size, one peak-to-mean ratio — name it once in the
[scratchpad](./modelling.md#shared-quantities) and have both read it. Authoring
the same `lognormal` in both places gives you a design that understates its
correlated risk.
:::

### Elementwise operators

`.+`, `.-`, `.*`, `./` and `.^` are accepted for upstream compatibility. In
Optimist they behave identically to `+`, `-`, `*`, `/` and `^`, because
distribution algebra here is *always* elementwise over aligned draws. Prefer the
plain spellings.

## Domain namespaces

The runtime ships the laws a capacity model is built from, so a manifest states
one rather than reimplementing it. The mathematics, assumptions, and failure
modes for each are in [Laws and models](../reference/laws.md).

### `Little`

$L = \lambda W$. See [Utilisation and Little's Law](../reference/laws.md#utilisation-and-littles-law).

| Signature | Returns |
| --- | --- |
| `Little.occupancy(rate, residence)` | Work in flight, $L$ |
| `Little.residence(occupancy, rate)` | Time each unit stays, $W$ |
| `Little.rate(occupancy, residence)` | Throughput, $\lambda$ |

### `Queue`

See [Waiting time](../reference/laws.md#waiting-time) and
[Saturation and the fold](../reference/laws.md#saturation-and-the-fold).

| Signature | Returns |
| --- | --- |
| `Queue.utilisation(demand, capacity)` | $\rho = \lambda/\mu$; `Queue.utilization` is the same function |
| `Queue.mm1Wait(service, utilisation)` | Mean wait in an M/M/1 queue |
| `Queue.mmcWait(service, servers, utilisation)` | Mean wait in an M/M/c queue, via Erlang C |
| `Queue.erlangB(servers, offered)` | Blocking probability with no waiting room |
| `Queue.erlangC(servers, offered)` | Probability an arrival has to wait |
| `Queue.boundedLength(utilisation, capacity)` | Mean number waiting in an M/M/1/K queue |
| `Queue.boundedBlocking(utilisation, capacity)` | Probability a full M/M/1/K queue turns an arrival away |

### `Reliability`

See [Reliability](../reference/laws.md#reliability),
[Retries](../reference/laws.md#retries),
[Deadline races](../reference/laws.md#deadline-races) and
[Quorums](../reference/laws.md#quorums).

| Signature | Returns |
| --- | --- |
| `Reliability.retrySuccess(attempt, attempts)` | $1 - (1-p)^n$ |
| `Reliability.retryAttempts(attempt, attempts)` | Expected attempts, and therefore the amplification applied downstream |
| `Reliability.serialSuccess(step, steps)` | $p^k$ for $k$ required independent steps |
| `Reliability.deadlineSuccess(steps, service, deadline)` | Chance an Erlang-distributed request finishes in time |
| `Reliability.quorumSuccess(node, nodes, required)` | Chance at least $r$ of $n$ nodes succeed |
| `Reliability.quorumLatency(node, nodes, required)` | Mean time until the $r$th of $n$ replies arrives |

### `Slo`

See [Service levels](../reference/laws.md#service-levels).

| Signature | Returns |
| --- | --- |
| `Slo.errorBudget(rate, objective, window)` | Failures the objective permits over the window |
| `Slo.burnRate(observed, objective)` | Observed failure as a multiple of the allowance; one exhausts the budget exactly at the end of the window |

## Writing expressions inside a design

Expressions in a design see their component's own properties and channels, the
shared quantities declared ahead of them, the Squiggle standard library, and a
small set of reserved bindings the evaluator supplies.

| Binding | Available in | Is |
| --- | --- | --- |
| `t` | channels, transforms | Position on the simulation timeline |
| `dt` | channels, transforms | Length of one step |
| `steady` | channels | `true` when the solve is asking where the design rests |
| `prev` | channels | This component's channel values at the previous step |
| `in` | channels | Requests arriving on this component's inbound ports |
| `out` | channels | Responses returning on this component's outbound ports |
| `signal` | mutator transforms | The signal currently travelling along the relationship |
| `request` | mutator transforms | The request travelling caller to callee |
| `response` | mutator transforms | The response returning callee to caller |

`pi`, `e` and `infinity` are also always in scope.

::: warning `t` is a position, not a wall clock
`t` is the step index multiplied by the step length, which defaults to one. A
shipped example runs roughly `t = 0` to `t = 20`, so stage a behavioural shift
inside that range:

```squiggle
if t >= 5 && t < 15 then base_demand * surge_factor else base_demand
```

Treat it as "where on the timeline", not "how many seconds in". Thresholds
borrowed from a real clock — `3600`, `300` — will simply never be reached.
:::

A component cannot see what it publishes itself. `in.<port>.<signal>` is what
arrived and `out.<port>.<signal>` is what came back, so neither can be confused
for the response this component is sending.

Worked examples, straight from the shipped catalogue:

```yaml
# src/system/catalogue/queue.yaml — drain what is offered plus what was waiting
expression: min(arrivals + prev.backlog / dt, service_rate)
```

```yaml
# src/system/catalogue/queue.yaml — one law at rest, integration while moving
expression: >
  if steady
    then Queue.boundedLength(load, capacity)
    else min(max(prev.backlog + (arrivals * accepted_ratio - departures) * dt, 0), capacity)
```

```yaml
# src/system/catalogue/mutators/timeout.yaml — where latency becomes failure
expression: >
  signal.success * Reliability.deadlineSuccess(1,
  max(signal.latency, 0.000001), budget)
```

```yaml
# examples/metastable/_system.yaml — a surge staged on the timeline
expression: >
  if t >= surge_from && t < surge_until then base_demand * surge_factor
  else base_demand
```

::: tip Guard the divisor, do not test it
Every quantity is a sample set, so `if backlog >= capacity` has no single answer:
it is true in some draws and false in others. The catalogue writes arithmetic
instead — `max(departures, 0.000001)`, `min(..., 1)` — which stays defined per
draw and reports the *share* of draws that saturated.
:::

## Determinism and sampling

A run resets its ChaCha20 stream to the configured seed, so identical source and
configuration replay exactly on any machine.

| Control | Default | Notes |
| --- | --- | --- |
| `--seed` | `0` | Root of the random stream |
| `--samples` | `1000` | Draws carried through every uncertain quantity |
| HTTP `samples` | `2000` | Clamped to between 64 and 20,000 |

```sh
optimist solve ./design --samples 10000 --seed 7
```

A thousand draws is enough for a mean; ten thousand is more honest about a
ninetieth percentile. `System.sampleCount()` reports the count in force, which is
occasionally useful when an expression builds a list of the same length.

Because a comparison uses one seed and one set of draws for both sides, a
difference between a baseline and an intervention is a difference in the design
rather than a difference in the draws. See
[Uncertainty](./uncertainty.md#determinism) for the rest.

## Errors and diagnostics

Optimist lints an expression before it runs it, so most mistakes are reported in
the field you are typing into. These are the messages you are most likely to
meet.

| You wrote | Optimist says |
| --- | --- |
| `missing(1)` | `unknown identifier` |
| `missing + 1` | `unknown identifier 'missing'` |
| `1 + true` | `operator '+' does not accept Number and Boolean` |
| `if 1 then 2 else 3` | `condition must be Boolean` |
| `mean('x')` | `no overload of 'mean' accepts (String)` |
| `List.length([1], 2)` | `no overload of 'List.length'` |
| `List.missing([1])` | `unknown builtin 'List.missing'` |
| `f(x) = x; f(1, 2)` | `function expects 1 arguments, received 2` |
| `[3,5,8][1.8]` | `cannot index Array with Number` |
| `[3,5,8][10]` | `array index is out of bounds` |
| `{a: 1}.b` | `dictionary has no key 'b'` |
| `List.first([])` | `list must not be empty` |
| `normal(0, 0)` | `standard deviation must be greater than zero` |
| `x :: m = 1; y :: s = 2; x + y` | `incompatible units m and s` |
| `rate :: rps = 100; wait :: s = 0.2; occupancy :: s = rate * wait` | `declared unit s does not match inferred unit op` |
| `f(x: Number.rangeDomain(0,1)) = x; f(2)` | `outside its declared domain` |
| `0 to 5` | `'to' requires 0 < low < high` |

::: warning Coming from upstream Squiggle
Optimist implements the language, not the notebook around it. There is no
`Plot.*`, no calculators, no visual output of any kind — the workbench draws the
charts instead. Decorator syntax `@name(...)` parses, but there are no builtin
decorators, so `@name('x')` fails with `unknown decorator 'name'`; upstream's
documentation and display tags have no equivalent here. `import '...' as x`
parses but resolves only against modules registered through the Rust API, so it
is not usable from a design directory. `Sym.pointMass`, `Sym.poisson` and
`Sym.binomial` are globals only in Optimist.
:::

## Cheat sheet

| Want | Write |
| --- | --- |
| A certainty | `0.02` |
| A belief with a 90% interval | `40 to 120` |
| A latency anchored on its median | `0.01 * lognormal(0, 0.35)` |
| A share bounded to $[0,1]$ | `beta(9, 1)` |
| A fast path and a slow path | `mixture(fast, slow, [0.9, 0.1])` |
| A percentage | `95%` |
| Thousands, millions, billions | `2k`, `4M`, `1.5B` |
| A duration | `90minutes` |
| Something staged on the timeline | `if t >= 5 && t < 15 then a else b` |
| The previous step's value | `prev.backlog` |
| What arrived on a port | `in.requests.rate` |
| What came back from a dependency | `out.calls.latency` |
| Rest versus motion | `if steady then ... else ...` |
| A local name inside one quantity | `{ base = ...; base * 2 }` |
| A summary | `d -> quantile(0.95)` |
| A safe divisor | `max(divisor, 0.000001)` |
