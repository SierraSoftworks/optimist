# Choosing a distribution

Every property in a design is Squiggle source, so every property is an
opportunity to say how sure you are. That is useful and it is also immediately
awkward: the field wants an expression and all you have is "about forty
milliseconds, sometimes two hundred".

This page is the answer to that. The first half says which family fits which
kind of input and how to parameterise it from something you actually know. The
second half gives you starting numbers, borrowed with thanks from
[Simon Eskildsen's napkin-math project][napkin].

[napkin]: https://github.com/sirupsen/napkin-math

::: tip
[Uncertainty](./uncertainty.md) covers how draws are carried, aligned, and read
back; [the Squiggle language guide](./language.md) covers the syntax. This page
covers the modelling judgement between the two.
:::

## Reach for this

| If you are modelling | Reach for | Because |
| --- | --- | --- |
| A service time or latency | `k * lognormal(0, σ)` or `a to b` | Positive, right-skewed, multiplicative |
| A payload or record size | `lognormal`, or `mixture` if bimodal | Same shape, usually wider |
| A cache hit ratio, success rate, or any `share` | `beta(α, β)` | Support is exactly $[0,1]$ |
| An all-or-nothing event | `bernoulli(p)` | Two outcomes, no middle |
| A count in a fixed window | `poisson(λ)` | Independent arrivals |
| A count out of $n$ tries | `binomial(n, p)` | Bounded above by $n$ |
| A configuration value | a bare number | It is set, not sampled |
| A policy range with hard ends | `uniform(a, b)` or `triangular(a, m, b)` | Bounded, not believed |
| A belief with no hard ends | `a to b` | A 90% credible interval |
| A fast path and a slow path | `mixture(...)` | Two processes, not one |
| A memoryless inter-arrival or service gap | `exponential(rate)` | Constant hazard |
| A sum of $k$ exponential stages | `gamma(k, θ)` | Erlang, by construction |

## What Optimist ships

These are the constructors the runtime implements. Nothing else exists, and a
name that is not on this list is a lint error rather than a silent zero.

| Constructor | Parameters | Support |
| --- | --- | --- |
| `normal(mean, stdev)` | mean, standard deviation | $(-\infty, \infty)$ |
| `lognormal(mu, sigma)` | $\mu$ and $\sigma$ **of the log** | $(0, \infty)$ |
| `uniform(low, high)` | bounds | $[a, b]$ |
| `beta(alpha, beta)` | positive shape parameters | $[0, 1]$ |
| `triangular(low, mode, high)` | bounds and peak | $[a, b]$ |
| `exponential(rate)` | rate $\lambda$ | $[0, \infty)$ |
| `gamma(shape, scale)` | $k$, $\theta$ | $[0, \infty)$ |
| `logistic(location, scale)` | centre, scale | $(-\infty, \infty)$ |
| `cauchy(location, scale)` | centre, scale | $(-\infty, \infty)$ |
| `bernoulli(p)` | probability | $\{0, 1\}$ |
| `binomial(trials, p)` | $n$, $p$ | $\{0 \ldots n\}$ |
| `poisson(rate)` | mean count $\lambda$ | $\{0, 1, 2, \ldots\}$ |
| `pointMass(value)` | a certainty | one atom |
| `mixture(...)`, `mx(...)` | components and weights | union of components |
| `a to b` | 5th and 95th percentiles | $(0, \infty)$, lognormal |
| `truncate(d, low, high)` | distribution and bounds | $[low, high]$, conditioned |

::: warning No Pareto, no Weibull
Neither is implemented. When you need a genuinely heavy tail, raise $\sigma$ on
a lognormal or bolt a slow component onto a mixture — both are covered under
[service time](#service-time-and-latency) below.

Avoid `cauchy` in a capacity model. It has no mean, so `mean()` on it raises a
runtime error and any figure derived from it is undefined rather than merely
wide.
:::

::: details Which names live under `Sym.` and `Dist.`
The bare global names above always work and are what the shipped examples use.
The namespaced spellings cover a subset: `Sym.` carries `normal`, `lognormal`,
`uniform`, `beta`, `cauchy`, `gamma`, `logistic`, `exponential`, `bernoulli`, and
`triangular`; `Dist.` carries `make`, `normal`, `lognormal`, `uniform`, `beta`,
`gamma`, `mixture`, and the sampling helpers. `poisson`, `binomial`, and
`pointMass` are globals only. `optimist catalogue <design> --output json` lists
exactly what your build accepts.
:::

## Request volume and arrival rate

A `client`'s `request_rate` is a rate in operations per second, not a count, so
it is nearly always a shared quantity with a spread on it rather than a Poisson
draw. Put the uncertainty on the *rate parameter*, and reach for `to` because
that is the form your uncertainty arrives in:

```yaml
scratchpad:
  - name: peak_rate
    expression: 700 to 1300     # 90% sure the peak lands in here
    unit: op/s
    summary: Requests per second at the daily peak.
```

A diurnal profile is not a distribution. It is a known shift in demand, and every
expression may read `t`, the position the solve has reached on the simulation
timeline (see [solving and bottlenecks](./analysis.md#time)):

```squiggle
if t >= 5 && t < 15 then peak_rate else 300
```

::: tip Peak-to-mean is the number worth naming
Most teams know their daily total and their peak-to-mean ratio far better than
they know the peak. Name the ratio, and an intervention can argue with it.

```yaml
- name: peak_ratio
  expression: '3 to 6'
  unit: '1'
- name: peak_rate
  expression: mean_rate * peak_ratio
  unit: op/s
```
:::

`poisson(λ)` belongs where the quantity really is a count in a window — messages
in a batch window, events per second when the discreteness matters.

::: warning Poisson understates burstiness
Poisson assumes arrivals are independent, which real traffic is not: retries
arrive together, cron fires together, a mobile client wakes a million sessions on
the same minute boundary. Its variance is pinned to its mean, so it cannot
express a burst.

Model burstiness as uncertainty in the *rate* instead. A Poisson count with an
uncertain rate produces an overdispersed mixed-Poisson process.
:::

## Service time and latency

Lognormal is the default and it earns the position. A response time is the
product of many small independent effects — a scheduler slice, a cache miss, a
lock held a little longer — and a product of many small factors tends toward
lognormal for the same reason a sum tends toward normal. It is positive
everywhere and skewed right, which is what every latency histogram looks like.

The shipped examples anchor on the median and multiply by a unit-median
lognormal, which keeps the number you measured visible in the source:

```yaml
# examples/saturation/components/orders.yaml
properties:
  service_time: 0.01 * lognormal(0, 0.15)
```

$\sigma$ is the only dial. It is the standard deviation of the *log*, so the
ratio between p90 and the median is $e^{1.28\sigma}$:

| $\sigma$ | p90 / median | Reads as |
| --- | --- | --- |
| `0.1` | 1.14× | A tight, well-behaved service |
| `0.35` | 1.57× | An ordinary request path |
| `0.7` | 2.5× | Contended, or several dependencies deep |
| `1.2` | 4.6× | A visible tail |
| `2.0` | 12.8× | Heavy tail; check this is real |

`exponential(rate)` is the memoryless alternative: the time remaining is
independent of how long you have already waited. That is a strong claim, usually
wrong for application work and right for gaps between independent events. It is
also broad, with a coefficient of variation of one. That makes its variability
easy to interpret, but does not make it a conservative bound on every service
time distribution with the same mean.

::: warning `normal` is almost always wrong for a latency
Its support includes negative numbers, so `normal(0.05, 0.03)` produces draws in
which the request finished before it arrived. And it is symmetric, so it claims
the fast tail is as long as the slow tail, which no queueing system has ever
done. The symptom is a design that looks fine at the mean and reports impossible
percentiles.
:::

### Bimodality and heavy tails

A cache-hit path and a cache-miss path are two processes, and averaging them
produces a figure that describes neither. Say so with a mixture:

```squiggle
mixture(0.002 * lognormal(0, 0.2), 0.05 * lognormal(0, 0.4), [0.9, 0.1])
```

Ninety percent of calls land near 2 ms, ten percent near 50 ms. The mean of that
is about 7 ms, and a model built on 7 ms gets the tail wrong in both directions.
A rare disaster — a GC pause, a lock convoy, a cold shard — is the same shape
with a smaller weight, and it is a better way to express a heavy tail than a
lognormal with a $\sigma$ nobody can defend, because it names the rare event and
lets you argue about how rare it is:

```squiggle
mixture(0.01 * lognormal(0, 0.3), 2 to 20, [0.999, 0.001])
```

::: tip Prefer two ports to one mixture where you can
If the two paths hit different dependencies, wire them as two relationships from
different outbound ports. A mixture describes the shape; separate
[ports](./modelling.md#ports) let the fast and slow paths be sized apart.
:::

## Payload and record size

Same shape, same reasoning, usually wider — record sizes span orders of magnitude
because the things they describe do. A small-record and large-blob split is the
commonest bimodality in storage:

```yaml
properties:
  record_size: 800 to 12000    # bytes per record
  # two populations, not one:
  # record_size: mixture(200 to 2000, 500e3 to 20e6, [0.97, 0.03])
```

::: warning The mean is a poor summary when the tail carries the bytes
Three percent of records at five megabytes contribute more total volume than the
other ninety-seven percent combined. A `datastore` sized on the mean record meets
its `transfer_limit` and `volume_limit` long before its `operation_limit`, and
the model only tells you that if the spread is in it.
:::

## Bandwidth and link speed

`bandwidth` on a relationship is **bytes per second**, matched against the
request and reply payloads together. A link speed is usually a stated figure, so
state it:

```yaml
outgoing:
  - to: orders
    bandwidth: '1.25e8'   # 1 Gbps, in bytes per second
    latency: '0.00025'    # a round trip inside the region
```

| Nameplate | Bytes per second |
| --- | --- |
| 1 Gbps | `1.25e8` |
| 10 Gbps | `1.25e9` |
| 25 Gbps | `3.125e9` |

Real links do not deliver their nameplate. Put the downside in and leave the top
end alone with `triangular(6e7, 1.1e8, 1.25e8)` — the nameplate is a ceiling, not
a belief.

::: tip Bandwidth left unset is unlimited
The default is `infinity`, because a speed nobody stated is a speed nobody meant
to constrain. It is also why a design whose operation rates all fit can still be
wrong: nothing checks the bytes until somebody writes a number down.
:::

## Hit ratios, success rates, and anything called a `share`

Beta is a flexible continuous family on $[0, 1]$. Its two parameters are positive
shapes; one useful way to derive them is from counts. If you saw $h$ hits out of
$n$ requests and use a uniform prior, set $\alpha = h + 1$ and
$\beta = n - h + 1$.

```yaml
scratchpad:
  - name: cache_hits
    expression: beta(85, 16)   # 84 hits in 99 requests, plus the uniform prior
    unit: share
    summary: Share of reads served from cache.
```

That is $0.84$ on average with a p10–p90 of roughly $0.79$ to $0.89$. Measure
more and the interval narrows without you having to invent a standard deviation.

| Belief | Expression |
| --- | --- |
| "Around 50%, could be anything" | `beta(2, 2)` |
| "About 84%, from a decent sample" | `beta(85, 16)` |
| "About 90%, but I am guessing" | `beta(9, 1)` |
| "Certainly 95% because it is configured" | `0.95` |
| "It either works or it does not" | `bernoulli(0.97)` |

::: warning Never put a lognormal or a normal on a share
Both put mass outside $[0, 1]$, so some draws claim a hit ratio of 1.4 or a
success rate below zero. The `cache` behaviour clamps `hit_ratio` into the unit
interval so a mistyped setting cannot send negative demand downstream, which
makes the failure *silent*: the design solves, the shape is wrong, and the
piled-up mass at the clamp quietly makes your cache look better than it is.
`beta` cannot do this, because it cannot leave the interval.
:::

The [`share` unit](./uncertainty.md#units) marks exactly these quantities. A
`success` of `0.97` and a fan-out of `3` are both dimensionless, and only one of
them is ninety-seven percent of anything.

## Counts: retries, branches, replicas, quorum size

These are configuration far more often than they are measurements — three
attempts is three attempts. Where the count genuinely varies per request, use a
distribution over counts:

```squiggle
poisson(4)                          // items in a page, mean four
binomial(10, 0.3)                   // shards touched out of ten
mixture(1, 2, 5, [0.7, 0.2, 0.1])   // a discrete profile you measured
```

::: tip A fractional branch count is meaningful here
`fan-out` with `branches: '2.4'` does not fetch 0.4 of something. Optimist solves
rates, so a branch count multiplies a rate: 2.4 means 1000 requests per second
produce 2400 downstream calls per second, which is what a mixed workload does.
Use the mean when you do not care about the shape, and a distribution when you
do.
:::

## Failure probability and availability

Beta again. Small probabilities are hard to picture, so work in the count you
have actually seen.

| Belief | Expression | Roughly |
| --- | --- | --- |
| "One failure in the last thousand calls" | `beta(2, 1000)` | $\approx 0.002$ |
| "None in ten thousand, so far" | `beta(1, 10000)` | $\approx 0.0001$ |
| "Two nines, and I have measured it" | `beta(99, 1)` | $\approx 0.99$ |

`bernoulli(p)` is for the all-or-nothing event — a region is up or it is not, a
feature flag is on or off — where averaging is exactly the thing you must not do.
Half the draws see one world and half the other, and the converged spread reports
the difference between them.

## Capacity, pool size, and connection limits

Usually certainties, because they are configuration values somebody typed into a
file. Put uncertainty here only when the number is not yours to set:

| Situation | Expression |
| --- | --- |
| A fixed connection pool | `'8'` |
| An autoscaler between bounds | `uniform(8, 32)` |
| A shared host with noisy neighbours | `8 * beta(9, 1)` |
| A limit you suspect but have not confirmed | `600 to 1200` |

::: tip Certain does not mean uninteresting
A configuration value is the ideal thing to name in the scratchpad, because an
[intervention](./modelling.md#interventions) can rebind it. The checkout example
does exactly that with `pool_size`, and the comparison means something precisely
because nothing else about the design moved.
:::

## Retention, TTL, and working-set size

A retention policy has hard ends: thirty days is thirty days, and a tiered policy
is a range with a floor and a ceiling rather than a belief with tails.

```yaml
properties:
  retention: uniform(604800, 2592000)      # between 7 and 30 days
  # or, when you know where inside the range it usually sits:
  # retention: triangular(86400, 604800, 2592000)
```

Retention is the property people forget, and the `datastore` type makes the
consequence explicit: `volume = Little.occupancy(operations, retention) *
record_size`. Thirty days of a modest write rate is often the binding constraint
in a design where everybody is watching the compute pool.

## Anything you only know as a range

`a to b` is a 90% credible interval, and it is the operator you will use most.
It builds a **lognormal** whose 5th and 95th percentiles are exactly `a` and `b`,
so both operands must be positive and `a` must be less than `b`. Give it a zero
or a negative and it refuses rather than guessing.

```squiggle
0.02 to 0.2   // 5% chance below 20ms, 5% chance above 200ms
```

A lognormal is right for the things engineers put ranges on — times, sizes,
rates, all positive and all skewed — and it is why `to` is not a uniform. A range
you believe in has tails; a range with genuine hard bounds does not.

| You mean | Reach for |
| --- | --- |
| "I am 90% sure it lands between these" | `a to b` |
| "It cannot be outside these, and I know nothing else" | `uniform(a, b)` |
| "It cannot be outside these, and it usually sits here" | `triangular(a, m, b)` |
| "It is this, and it is configured to be this" | a bare number |

## Turning a belief into parameters

Nobody has intuition for $\mu$ and $\sigma$ of a *log*. Do not try to acquire
any; state the percentiles and let the constructor solve for them. The accepted
pairs are `{p5, p95}`, `{p10, p90}`, and `{p25, p75}`, on both `normal` and
`lognormal`; both also accept `{mean, stdev}`, which for a lognormal describes
the *value* rather than its log.

```squiggle
Sym.lognormal({p10: 0.02, p90: 0.2})   // eighty percent of the time
Sym.lognormal({p5: 0.015, p95: 0.3})   // ninety percent of the time
Sym.normal({p5: -2, p95: 4})           // works for normal too
```

::: warning `beta` takes `{mean, stdev}` only
There is no percentile form for `beta`. Give it counts, or give it a mean and a
standard deviation — a percentile dictionary is a runtime error.
:::

So, working through "usually about 40 ms, sometimes 200 ms":

1. **Confirm what "usually about 40 ms" means.** If it describes the middle
  observed call, treat 40 ms as the median; if it is an average, do not silently
  reinterpret it.
2. **"Sometimes 200 ms"** is a tail. If "sometimes" means one call in ten, it is
  p90; if it means one in a hundred, the spread is much wider.
3. If 40 ms is the median, a lognormal with median $m$ and p90 $q$ has
  $\sigma = \ln(q/m) / 1.2816$, so here
  $\sigma = \ln(5) / 1.2816 \approx 1.26$. Write it either way:

```squiggle
0.04 * lognormal(0, 1.26)              // anchored on the median
Sym.lognormal({p10: 0.008, p90: 0.2})  // stated as percentiles
```

These are the same distribution. The anchored form is easier to rebind — an
intervention that halves the median touches one number — and the percentile form
is easier to defend to whoever gave you the figures.

Notice what the shape decided for you: a median of 40 ms with a p90 of 200 ms
implies a p10 of 8 ms. If that fast tail is wrong, your "sometimes" was rarer
than one in ten, and the honest fix is a mixture rather than a smaller $\sigma$.

## Correlation: one draw, shared

**This is the most common modelling error in a capacity model, and it is not
close.**

Two components that read the same scratchpad entry see the *same draw*. Two
components that each author their own `lognormal(-4.6, 0.35)` do not, and the
design silently assumes they are independent.

```yaml
# Wrong: two components each authoring their own record size.
# components/orders.yaml
properties:
  record_size: 800 to 12000
# components/archive.yaml
properties:
  record_size: 800 to 12000
```

```yaml
# Right: one fact about the system, stated once, referred to twice.
# _system.yaml
scratchpad:
  - name: record_size
    expression: 800 to 12000
    unit: B/op
    summary: Bytes in one order record, everywhere it is stored.

# components/orders.yaml
properties:
  record_size: record_size
```

The consequence is not subtle. Independent draws average out: with two of them,
the chance that both land in the top decile is one in a hundred instead of one in
ten. A design built that way reports a comfortable tail for a system whose parts
are all large together, all slow together, and all saturated together — which is
what an incident is.

::: tip Anything two components could plausibly share, share
Peak demand, record size, peak-to-mean ratio, a dependency's health, the hardware
generation. If two properties would move together in the real system, derive them
from one named quantity. Derived entries keep the correlation, so this is enough:

```yaml
- name: base_latency
  expression: 0.004 * lognormal(0, 0.2)
- name: index_latency
  expression: base_latency * 1.5
```
:::

Correlated failure is the same problem wearing a different hat. Every function in
the [`Reliability`](./uncertainty.md#reliability) namespace assumes independence
between attempts or nodes, and that assumption is optimistic: attempts against a
saturated dependency fail together. Express that through a shared upstream
quantity, not by trusting the formula.

## Why a mean-only model is optimistic

Queueing delay is convex in utilisation. M/M/1 waiting time goes as

$$W = \frac{S\,\rho}{1 - \rho}$$

and the $1 - \rho$ in the denominator bends the curve upward: the delay added by
going from 80% to 90% utilisation is far larger than the delay saved by going
from 80% to 70%. Jensen's inequality then says that for a convex $f$,

$$\mathbb{E}[f(X)] \geq f(\mathbb{E}[X])$$

so the average delay across an uncertain utilisation *exceeds* the delay at the
average utilisation. Optimist asserts this as a property —
`uncertain_utilisation_raises_mean_queueing_delay` in
`tests/squiggle_system_prelude.rs` checks that the mean of
`Queue.mm1Wait(0.01, uniform(0.3, 0.9))` is more than five percent above
`Queue.mm1Wait(0.01, 0.6)`.

```squiggle
Queue.mm1Wait(0.01, 0.6)                      // the delay at the mean
mean(Queue.mm1Wait(0.01, uniform(0.3, 0.9)))  // strictly larger
```

Two things follow, and both are the point of this whole page:

- **A mean-only model understates delay**, systematically and by more the closer
  you run to the ceiling.
- **A mean-only model understates the chance of saturation** entirely, because it
  has no draws to count. A constraint at 60% mean utilisation can still exceed
  its limit in a fifth of draws, and that share is the figure worth acting on.
  [Analysis](./analysis.md) covers how it is reported.

## How much spread to use

Both failure modes are real, and they are not symmetric.

| Too narrow | Too wide |
| --- | --- |
| The model is confidently wrong | The model is uselessly vague |
| Saturation looks impossible until it happens | Everything looks possible |
| Nobody argues, because it looks precise | Nobody acts on it |
| Costs you an incident | Costs you a conversation |

Overconfidence is the worse of the two, so when you are torn, go wider. A wide
interval invites the measurement that narrows it; a narrow one invites nothing.
If you measured it, use the measurement's spread; if you looked it up, `to`
across the figures you found; if you guessed, guess an order of magnitude
(`0.01 to 0.1`, not `0.03 to 0.05`); if you configured it, it is a number.

::: tip A wide converged distribution is a finding
When the answer comes back broad and the input was not, you have found a fold:
some draws crossed into congestion and some did not, and the spread is a genuine
mixture rather than noise to be smoothed away. Narrowing the input to make the
output look tidy destroys exactly the information you built the model to get. See
[solving and bottlenecks](./analysis.md).
:::

## Sanity-checking what you wrote

Three checks, cheapest first.

**Read the preview.** Every field taking an expression previews the density, p10,
median, and p90 of what it evaluates to while you type. If the median is not the
number you meant, stop there.

**Ask the runtime**, with the same statistics functions the reports use:

```squiggle
{
  service = 0.04 * lognormal(0, 1.26)
  [quantile(service, 0.1), median(service), quantile(service, 0.9)]
}
```

**Solve it**, and read the interval rather than the mean:

```sh
optimist solve ./examples/checkout --samples 10000
```

```text
api  capacity  685.1550 [450.9287 .. 947.7374]
```

A thousand draws is enough for a mean; ten thousand is more honest about a
ninetieth percentile. If the interval is implausible, the mean was hiding it.

---

## Starting numbers

The rest of this page is a reference of order-of-magnitude figures for a first
model, reproduced from **[Simon Eskildsen's napkin-math project][napkin]**, where
they were measured and where they are maintained. Credit and any corrections
belong there.

::: warning These are anchors, not measurements
Every figure below is rounded for memorisation rather than accuracy, and
hardware, cloud provider, and workload move all of them. Use one to get a first
model solving, then replace it with something you measured.

Putting a distribution on a borrowed number is how you say you borrowed it. A
looked-up SSD read of 100 µs written as `50e-6 to 300e-6` is honest; written as
`0.0001` it is a claim you cannot support.
:::

### CPU, memory, and hashing

| Operation | Latency | Throughput |
| --- | --- | --- |
| Sequential memory R/W (64 B) | 0.5 ns | 20 GiB/s single thread, 200 GiB/s threaded |
| Random memory R/W (64 B) | 20 ns | 3 GiB/s |
| Hashing, not crypto-safe (64 B) | 10 ns | 5 GiB/s |
| Hashing, crypto-safe (64 B) | 100 ns | 1 GiB/s |
| System call | 300 ns | — |
| Context switch | 10 µs | — |

### Storage and data handling

| Operation | Latency | Throughput |
| --- | --- | --- |
| Sequential SSD read (8 KiB) | 1 µs | 8 GiB/s |
| Sequential SSD write, no fsync (8 KiB) | 2 µs | 3 GiB/s |
| Random SSD read (8 KiB) | 100 µs | 70 MiB/s |
| Sequential SSD write, +fsync (8 KiB) | 300 µs | 30 MiB/s |
| Sequential HDD read (8 KiB) | 10 ms | 250 MiB/s |
| Random HDD read (8 KiB) | 10 ms | 0.7 MiB/s |
| Fast serialisation / deserialisation | — | 1 GiB/s |
| Serialisation / deserialisation (e.g. JSON) | — | 100 MiB/s |
| Decompression | — | 1 GiB/s |
| Compression | — | 500 MiB/s |
| Sorting (64-bit integers) | — | 500 MiB/s |
| MySQL / Memcached / Redis query | 500 µs | — |
| Blob storage GET, 304 if-not-match | 30 ms | — |
| Blob storage GET, one connection (128 KiB) | 80 ms | 100 MiB/s |
| Blob storage LIST | 100 ms | — |
| Blob storage PUT, one connection (128 KiB) | 200 ms | 100 MiB/s |

The sequential-to-random ratio is the whole story: 100× on SSD, closer to 350×
on spinning disk. A design whose access pattern is uncertain should say so with a
mixture rather than an average.

### Network

| Path | Latency | Throughput |
| --- | --- | --- |
| Same zone / inside VPC | — | 10 GiB/s |
| Outside VPC | — | 3 GiB/s |
| TCP echo server (32 KiB) | 50 µs | 500 MiB/s |
| Proxy hop (Envoy, Nginx, HAProxy, ProxySQL) | 50 µs | — |
| Within same region | 250 µs | 2 GiB/s |
| Premium network within zone/VPC | 250 µs | 25 GiB/s |
| NA Central ↔ East | 25 ms | 25 MiB/s |
| NA Central ↔ West | 40 ms | 25 MiB/s |
| NA East ↔ West | 60 ms | 25 MiB/s |
| EU West ↔ NA East | 80 ms | 25 MiB/s |
| EU West ↔ NA Central | 100 ms | 25 MiB/s |
| EU West ↔ Singapore | 160 ms | 25 MiB/s |
| NA West ↔ Singapore | 180 ms | 25 MiB/s |

### Compression ratios

| What | Ratio |
| --- | --- |
| HTML | 2–3× |
| English text | 2–4× |
| Source code | 2–4× |
| Executables | 2–3× |
| RPC | 5–10× |
| SSL | −2% |

::: tip Another × of compression costs about 10× the speed
2× on English Wikipedia runs at roughly 200 MiB/s, 3× at roughly 20 MiB/s, 4× at
roughly 1 MB/s. That is the exchange rate when you trade bandwidth for CPU.
:::

### How to use these

The advice is napkin-math's, and it applies here unchanged:

- **Do not overcomplicate.** More than six assumptions and you are making it
  harder than it needs to be.
- **Keep the units.** They are free checksums, which is why every Optimist
  property carries a unit annotation.
- **Calculate with exponents.** Get $e$ right in $c \times 10^{e}$ and worry
  about $c$ later.
- **Decompose.** Write down the things you can guess at until they add up.

Then measure. A borrowed figure is where a model starts, not where it stops.

### From a napkin number to an Optimist property

| Napkin figure | Optimist property | Expression |
| --- | --- | --- |
| Same-region round trip, 250 µs | relationship `latency` | `'0.00025'` |
| Cross-region EU West ↔ NA East, 80 ms | relationship `latency` | `'0.08'` |
| 1 Gbps link | relationship `bandwidth` (B/s) | `'1.25e8'` |
| 10 GiB/s same-zone network | relationship `bandwidth` (B/s) | `'1.07e10'` |
| Random SSD read, 100 µs | `datastore` `service_time` | `100e-6 * lognormal(0, 0.4)` |
| Redis / MySQL query, 500 µs | `datastore` `service_time` | `0.0005 * lognormal(0, 0.4)` |
| Blob GET, 80 ms | `datastore` `service_time` | `0.04 to 0.16` |
| Random SSD read, 70 MiB/s | `datastore` `transfer_limit` (B/s) | `'7.34e7'` |
| Crypto hash at 1 GiB/s over an 8 KiB body | `compute` `service_time` | `8192 / 1.07e9` |
| JSON parse at 100 MiB/s over a 4 KiB body | `compute` `service_time` | `4096 / 1.05e8` |
| Proxy hop, 50 µs | `load-balancer` `overhead` | `'0.00005'` |
| Context switch, 10 µs | ignore it | it is noise beside everything above |

Two conversions worth writing down, because getting them wrong is an eight-fold
error: `bandwidth` and `transfer_limit` are **bytes** per second while link
speeds are quoted in **bits**, and napkin-math's throughputs are binary, so
1 GiB/s is `1.07e9` B/s rather than `1e9`.

```yaml
# components/orders.yaml — a store whose figures came off the napkin.
id: orders
type: datastore
properties:
  service_time: 0.0005 * lognormal(0, 0.4)   # 500µs query, with a real spread
  operation_limit: '20000'
  transfer_limit: '7.34e7'                   # 70 MiB/s of random reads
  volume_limit: '2e12'
  record_size: 800 to 12000
  retention: '2592000'                       # thirty days
```

Every one of those is a starting assumption. Record which figures were measured
and which were borrowed in the summary of the scratchpad entry, component, or
relationship that supplies them; use the spread to record how much difference
the uncertainty makes.
