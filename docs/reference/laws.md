# Laws and models

Optimist does not guess. Every figure a solve reports is produced by a named law
with stated assumptions, evaluated over draws rather than over point estimates.
This page says which law, where it is applied, how it is parameterised, what it
assumes, and how it fails when the assumption does not hold. It is the
mathematical companion to the [shipped catalogue](./catalogue.md), which
describes the same components in prose.

## Summary

| Law | Where it is applied | Optimist name | Key assumption |
| --- | --- | --- | --- |
| Little's Law | `compute.capacity`, `compute.concurrency`, `compute.held_downstream`, `load-balancer.connections`, `datastore.records`, `queue.wait`, every wire's wait | `Little.occupancy`, `Little.residence`, `Little.rate` | Stability, and the same averaging interval for all three quantities. Nothing else. |
| M/M/1 waiting time | `Queue.mm1Wait` in project models | `Queue.mm1Wait` | Poisson arrivals, exponential service, one server, unbounded queue, steady state. |
| M/M/1 residence stretch | `datastore.latency` | written inline as `service_time / max(1 - concurrency, 0.001)` | As M/M/1, with a guard replacing the pole. |
| M/M/c waiting time (Erlang C) | `Queue.mmcWait` in project models | `Queue.mmcWait`, `Queue.erlangC` | As M/M/1 with $c$ identical pooled servers and one shared queue. |
| Erlang B (loss system) | `Queue.erlangB` in project models | `Queue.erlangB` | No waiting room: a blocked arrival leaves rather than queues. |
| M/M/1/K bounded queue | The queue on **every relationship**; `queue.backlog`, `queue.accepted_ratio` | `Queue.boundedLength`, `Queue.boundedBlocking` | Finite buffer of depth $K$; excess demand becomes refusal, not unbounded delay. |
| Serialisation and propagation | Every relationship carrying bytes | the `carriage` model | Bytes are put on the wire at a constant speed; one reply per request. |
| Retry composition | `retry` behaviour | `Reliability.retrySuccess`, `Reliability.retryAttempts` | Attempts fail independently. |
| Serial reliability | `Reliability.serialSuccess` in project models | `Reliability.serialSuccess` | Steps fail independently; every step is required. |
| Deadline race (Erlang/gamma CDF) | `timeout` behaviour | `Reliability.deadlineSuccess` | Each step takes an exponential time; steps are independent. |
| Quorum availability (binomial tail) | `quorum.success_rate` | `Reliability.quorumSuccess` | Nodes fail independently. |
| Quorum latency (order statistic) | `quorum.quorum_wait` | `Reliability.quorumLatency` | Exponential, independent node response times. |
| Fan-out cost | `aggregator`, `fan-out` behaviour | catalogue expressions | Every branch is required; branch success multiplies. |
| Error budget and burn rate | Project models and SLO reporting | `Slo.errorBudget`, `Slo.burnRate` | Eligible operations are counted uniformly over the window. |
| KL divergence and log score | Forecast scoring in project models | `Dist.klDivergence`, `Dist.logScore` | Absolutely continuous densities; Monte Carlo estimate with no error bound. |
| Probability of binding | `optimist bottlenecks` ranking | `probability_of_binding` | Draws are the model's own ensemble; no distributional assumption. |

---

## Utilisation and Little's Law

For any stable system observed over a long interval, the mean number resident
equals the mean arrival rate multiplied by the mean time each arrival spends
there:

$$L = \lambda W$$

Read it three ways. Given a rate and a residence time you get occupancy; given
occupancy and a rate you get residence time; given occupancy and residence time
you get throughput. All three are the same identity, and Optimist exposes each
one so a model states which reading it means.

| Function | Returns | Definition |
| --- | --- | --- |
| `Little.occupancy(rate, residence)` | Work in flight, $L$ | $\lambda W$ |
| `Little.residence(occupancy, rate)` | Time each unit stays, $W$ | $L / \lambda$ |
| `Little.rate(occupancy, residence)` | Throughput, $\lambda$ | $L / W$ |

The law is distribution-free. It assumes only that the system is stable, that
arrivals and departures balance over the averaging interval, and that all three
quantities are measured over that same interval. It does not assume Poisson
arrivals, exponential service, a queueing discipline, or independence. That is
why it applies unchanged to connections held on a balancer, records retained in
a store, and messages resident in a buffer.

| Component | Channel | Expression |
| --- | --- | --- |
| `compute` | `capacity` | `Little.rate(servers, hold_time)` |
| `compute` | `concurrency` | `Little.occupancy(rate, residence)` |
| `compute` | `held_downstream` | `Little.occupancy(calls, dependency_wait)` |
| `load-balancer` | `connections` | `Little.occupancy(forwarded, latency)` |
| `datastore` | `records` | `Little.occupancy(operations, retention)` |
| `queue` | `wait` | `Little.residence(backlog, max(departures, 0.000001))` |

`compute.capacity` is the load-bearing one. A pool's sustainable throughput is
its concurrent slots divided by the time each request holds one, and the hold
time includes whatever the request spent blocked on a dependency. That single
coupling is what lets somebody else's latency problem become your saturation
problem, and it is derived below.

Utilisation is the ratio a capacity model is read against:

$$\rho = \frac{\lambda}{\mu} = \frac{\text{demand}}{\text{capacity}}$$

`Queue.utilisation(demand, capacity)` — spelled `Queue.utilization` as well —
returns exactly that, and refuses a capacity of zero rather than returning an
infinity every quantity downstream would then carry.

::: warning Assumption: stability and a shared interval
Little's Law says nothing about a system whose queue is growing. Applied to an
interval during which the backlog rose, $L$, $\lambda$ and $W$ describe three
different things and the identity is an accounting error rather than a result.
Optimist evaluates it per draw at a solved fixed point, which is where the
premise holds; in `--transient` mode the same expressions are evaluated at each
step, and a step during which a queue is filling reports the instantaneous
figures rather than a long-run average.
:::

---

## Waiting time

Waiting-time results are not distribution-free. They describe M/M/c: Poisson
arrivals, exponential service, $c$ identical servers, one unbounded
first-come-first-served queue, and steady state.

### M/M/1

Mean time waiting before service begins, and mean total time in the system:

$$W_q = \frac{\rho S}{1 - \rho}, \qquad R = W_q + S = \frac{S}{1 - \rho}$$

`Queue.mm1Wait(service, utilisation)` returns $W_q$. Residence is deliberately
not returned directly, so a model states whether it means queueing delay or
total sojourn.

The $1/(1-\rho)$ pole is the whole story of a design near saturation. At
$\rho = 0.5$ a request waits as long as it is served; at $\rho = 0.9$ it waits
nine times as long; at $\rho = 0.99$, ninety-nine times. Utilisation is not a
budget you can spend to the last percent.

| $\rho$ | $W_q / S$ | $R / S$ |
| --- | --- | --- |
| 0.50 | 1.0 | 2.0 |
| 0.80 | 4.0 | 5.0 |
| 0.90 | 9.0 | 10.0 |
| 0.95 | 19.0 | 20.0 |
| 0.99 | 99.0 | 100.0 |

### M/M/c and Erlang C

Offered load in erlangs is $a = \lambda / \mu = \lambda S$, and utilisation is
$\rho = a / c$. Erlang B gives the blocking probability of a loss system with no
waiting room, evaluated by the numerically stable recursion

$$B(0, a) = 1, \qquad B(n, a) = \frac{a\,B(n-1, a)}{n + a\,B(n-1, a)}$$

which avoids the overflow of evaluating factorials directly. Erlang C gives the
probability that an arrival must wait in a delay system,

$$C(c, a) = \frac{B(c, a)}{1 - \rho\,(1 - B(c, a))}$$

and the mean waiting time before service begins is

$$W_q = \frac{C(c, a)\,S}{c\,(1 - \rho)}$$

`Queue.mmcWait(service, servers, utilisation)` computes $a = \rho c$ from its
arguments and evaluates that expression. `Queue.erlangB(servers, offered)` and
`Queue.erlangC(servers, offered)` expose the two probabilities directly and take
offered load in erlangs rather than a utilisation.

Pooling is the economy of scale this law reports. At the same utilisation, more
servers wait less, because the chance that every one of them is busy falls
faster than the count rises. The prelude test
`pooling_servers_reduces_waiting` holds the implementation to it across
1, 2, 4, 8 and 16 servers at $\rho = 0.8$, and
`one_server_agrees_between_the_general_and_special_case` holds `mmcWait` at
$c = 1$ to agree with `mm1Wait`.

### Saturation

No stationary result exists at $\rho \geq 1$: the queue grows without bound and
the mean is infinite. Utilisation is therefore clamped just below one at
$\rho_{\max} = 1 - 10^{-6}$, so a saturated queue yields a very large but finite
delay — around six orders of magnitude above its service time. That value is a
saturation sentinel, not a prediction. The honest reading is that demand has
exceeded capacity, not that a request will take that long.
`saturated_queues_report_a_large_finite_delay` pins the behaviour.

::: warning Assumption: exponential service
Exponential service has a coefficient of variation of one. It is more variable
than deterministic service, but it is not the most variable distribution for a
given mean. Under the corresponding M/G/1 assumptions, more regular service
queues less than this result predicts and service with a coefficient of
variation above one queues more. The exact M/M formulas do not apply outside
exponential service, so a design whose margin rests on the third significant
figure of a queueing delay is resting on the wrong thing.
:::

### Bounded queues: M/M/1/K

A real buffer has a depth. For a buffer holding at most $K$ operations at load
$\rho$, the stationary distribution is truncated geometric,
$p_n = (1-\rho)\rho^n / (1-\rho^{K+1})$, giving a mean length

$$L = \frac{\rho}{1-\rho} - \frac{(K+1)\rho^{K+1}}{1-\rho^{K+1}}$$

and a blocking probability, the last term of that distribution,

$$P_K = \frac{(1-\rho)\rho^{K}}{1-\rho^{K+1}}$$

At $\rho = 1$ both expressions are indeterminate. Every occupancy is then
equally likely, so the exact values are $K/2$ and $1/(K+1)$, and those are used
directly within $10^{-9}$ of that point to avoid catastrophic cancellation. Far
above saturation, where $\rho^{K+1}$ overflows, the length is $K$ and the
blocking probability is $1 - 1/\rho$.

`Queue.boundedLength(utilisation, capacity)` and
`Queue.boundedBlocking(utilisation, capacity)` expose both, clamped to
$[0, K]$ and $[0, 1]$ respectively.

The bound is what makes overload legible. An unbounded queue reports an
arbitrarily large delay, which is neither true of a system with a finite buffer
nor useful to anybody reading the result. A bounded queue turns excess demand
into failure — which is what a full socket buffer actually does.

---

## The queue on every relationship

A relationship in Optimist is not a label on an arrow. It is a buffer of finite
depth, fed at one rate and drained at another, and it is solved with the M/M/1/K
results above.

```text
caller ──▶ [ buffer, depth K ] ──▶ callee
             ρ = offered / served
             served = min(callee capacity, wire throughput)
```

What can be taken away is the slower of the two things in the way: the far end's
published `capacity`, and the operation rate the wire's own speed allows. A link
carrying more bytes than it can move backs up in front of a dependency with
cores to spare, and it is the wire that has to say so because neither end can
see it.

| Quantity the wire reports | Definition |
| --- | --- |
| `backlog` | $L$ from `boundedLength` at $\rho$ and depth $K$ |
| `wait` | $L / \text{served}$, which is Little's Law on the wire |
| `blocked` | $P_K$ from `boundedBlocking` |
| `transfer` | $\lambda \cdot (\text{request payload} + \text{response payload})$ |

The wait is added to the latency travelling back to the caller, and the blocking
probability reduces the success travelling with it: observed success becomes
$p \cdot (1 - P_K)$. That is why overload does not appear in `compute.success_rate`
or `load-balancer.success_rate` — work a fleet has no room for is refused on the
wire in front of it, and counting it in both places would charge one shortfall
twice.

### The carriage model

A link with a stated speed is a server as well as a buffer. With payload
$b$ bytes per operation (request plus reply) and speed $v$ bytes per second:

$$\text{throughput} = \frac{v}{b}, \qquad
\text{serialisation} = \frac{b}{v}, \qquad
\text{transit} = \text{propagation} + \frac{b}{v}$$

Transit is what an *idle* link costs; what a busy one costs on top of that is the
queue, solved from the throughput above rather than charged twice. A speed
nobody stated, a flow carrying nothing, and a speed of zero all leave the wire a
pure operation queue with infinite throughput and no transit cost.

This is what makes [`batch`](./catalogue.md#batch) legible. Dividing the
operation rate by $n$ while multiplying the payload by $n$ leaves the byte rate
exactly where it was: the right trade against a store limited by operations per
second, and no trade at all against one limited by bandwidth.

### Steady state and transient integration

The same equations either way; what differs is whether the backlog is asked to
balance or asked to move.

**`Steady`** (the default) solves the closed form above. It has no memory, so
where a design has more than one resting state this reports the one reachable
from nothing — a surge that would have tipped the design over and left it there
appears to have been survived.

**`Transient`** (`--transient`) integrates the backlog forward with explicit
Euler, using the previous step's rates:

$$\text{admissible} = \text{served} + \frac{\max(K - B_t, 0)}{\Delta t}, \qquad
a = \min(\lambda, \text{admissible})$$

$$B_{t+1} = \mathrm{clamp}\big(B_t + (a - \text{served})\,\Delta t,\; 0,\; K\big),
\qquad \text{refused} = \frac{\lambda - a}{\lambda}$$

Using the previous step's rates on purpose is what makes the pass explicit and
breaks the loop tying a queue's delay to the demand that delay is producing.

::: warning Assumption: the step is short against the drain time
Explicit Euler is only faithful while $\Delta t$ is short compared with the time
a queue takes to drain. Advance further than that and the integration overshoots
and oscillates — in the solver, not in the design. A drain that takes a while to
play out may need thousands of steps to follow. If a transient result
oscillates with a period of a couple of steps, halve `--step` before believing
it.
:::

---

## Saturation and the fold

This is Optimist's headline behaviour, and it emerges from two ordinary
modelling decisions meeting rather than from anything exotic.

A `compute` pool holds a worker for the whole of a request, including the time
spent blocked on a dependency:

```text
hold_time = service_time + dependency_wait
capacity  = Little.rate(servers, hold_time) = parallelism / hold_time
```

A `datastore` limited by simultaneous work stretches its latency by how much of
its concurrency is already spoken for:

```yaml
held:        in.operations.occupancy          # λ · L, published by the caller
concurrency: Queue.utilisation(held, concurrency_limit)
latency:     service_time / max(1 - concurrency, 0.001)
```

That last expression is the M/M/1 residence time $R = S / (1 - \rho)$ with the
occupancy share standing in for $\rho$.

### The closed form

Serving $\lambda$ operations each held for $L$ seconds puts $\lambda L$
operations in flight against a concurrency limit $C$, so
$\rho = \lambda L / C$. Substituting into the residence form:

$$L = \frac{S}{1 - \lambda L / C}$$

Multiply through and collect:

$$\lambda L^2 - C L + C S = 0
\qquad\Longrightarrow\qquad
L = \frac{C}{2\lambda}\left(1 \pm \sqrt{1 - \frac{4\lambda S}{C}}\right)$$

The discriminant is non-negative only while

$$4\lambda S \leq C$$

so the design has a steady state up to

$$\lambda^{*} = \frac{C}{4S}, \qquad L^{*} = 2S$$

and none at all beyond it. The lower root is the branch a system rests on; the
upper root is the unstable branch separating it from collapse. At the fold the
two roots meet, which is why a design at $\lambda^{*}$ has exactly twice its
idle latency and nowhere left to go.

For the shipped [`examples/saturation`](../examples/README.md) design, with
$S = 10\,\text{ms}$ and $C = 100$ connections:

| $\lambda$ (op/s) | $4\lambda S / C$ | $L$ | Stretch $L/S$ |
| --- | --- | --- | --- |
| 600 | 0.24 | 10.7 ms | 1.07× |
| 1 250 | 0.50 | 11.7 ms | 1.17× |
| 2 000 | 0.80 | 13.8 ms | 1.38× |
| 2 400 | 0.96 | 16.7 ms | 1.67× |
| 2 500 | 1.00 | 20.0 ms | 2.00× |
| 2 600 | 1.04 | no real solution | — |

$\lambda^{*} = 100 / (4 \times 0.01) = 2\,500$ operations per second. Note what
that number is built from: an idle latency and a pool size. Neither is a
throughput figure, and neither appears on a capacity plan. A design signed off
at 600 operations per second looks like it has four times the headroom it has.

::: warning Assumption: the guard is not a prediction
Past the fold there is no steady state, and `max(1 - concurrency, 0.001)` caps
the stretch at $1000 S$ rather than dividing by zero. That figure is a sentinel
saying "this design has no resting state here", not a latency anybody will
measure. Read the fold condition, not the number the guard produces.
:::

### Hysteresis and metastability

Two further ingredients turn a fold into a trap.

A **timeout** decides that a slow answer is a failure. A **retry** answers that
failure with more load. Together they make the effective arrival rate a rising
function of latency, so the loop gain climbs exactly when the system can least
afford it — and the load that tips the design over is not the load it recovers
at. [`examples/metastable`](../examples/README.md) is built entirely from
shipped catalogue types and demonstrates it; lengthening the deadline, the
reflex when requests start failing, makes it worse.

A **queue** with real depth adds state. What it holds it keeps, and draining it
is itself load on the dependency the user-facing path depends on. In
[`examples/queued-collapse`](../examples/README.md) a ten-second surge builds a
backlog still being worked off seventy seconds later, and the design reports two
different steady states at the same offered load with the queue empty in both —
differing only in what happened two minutes earlier.

Neither behaviour is asserted anywhere in the model. Both emerge from the
arithmetic above.

---

## Reliability

### Serial dependencies

A call that must complete $k$ independent steps succeeds with probability

$$P(\text{success}) = p^k$$

`Reliability.serialSuccess(step, steps)`. Reliability falls geometrically in
depth, which is why deep synchronous call chains fail far more often than any
single hop suggests: sixty-four steps at $p = 0.99$ leave 52%.

The same arithmetic appears structurally rather than as a function call. The
`success` signal aggregates by **product**, so a component reading
`out.dependencies.success` gets the chance that every dependency held.

### Retries

With attempt success probability $p$ and at most $n$ attempts, and assuming
attempts fail independently:

$$P(\text{success}) = 1 - (1 - p)^n$$

The expected number of attempts actually made matters more, because it is the
amplification a retry policy applies to downstream demand. Stopping at the first
success or the $n$th attempt gives a truncated geometric count with mean

$$\mathbb{E}[N] = \sum_{k=1}^{n} (1-p)^{k-1} = \frac{1 - (1-p)^n}{p}$$

which tends to $n$ as $p \to 0$.

The [`retry`](./catalogue.md#retry) behaviour applies both, and — this is the
important part — reads the success rate **coming back** rather than taking it as
a constant:

```yaml
requests:
  rate:     signal.rate * Reliability.retryAttempts(response.success, attempts)
responses:
  success:  Reliability.retrySuccess(signal.success, attempts)
  latency:  signal.latency * Reliability.retryAttempts(signal.success, attempts)
```

That closes a positive feedback loop. Failing means more load, and more load
means failing. It is what turns a transient fault into a retry storm the system
cannot leave on its own, and it is invisible unless the amplification is
modelled against the failures actually observed.
`retry_amplification_rises_as_a_dependency_fails` holds the implementation to a
call count that rises from 1.00 at $p = 0.999$ toward — but never past — the
budget as $p$ falls.

| $p$ | $\mathbb{E}[N]$ at $n = 3$ | Success |
| --- | --- | --- |
| 0.999 | 1.001 | 0.999999999 |
| 0.900 | 1.11 | 0.999 |
| 0.500 | 1.75 | 0.875 |
| 0.100 | 2.71 | 0.271 |
| 0.010 | 2.97 | 0.030 |

### Deadline races

A request performing $k$ sequential steps, each taking an exponential time with
mean $S$, finishes in the sum of $k$ exponentials. That sum is
Erlang-distributed with shape $k$ and rate $1/S$, so the probability of
finishing within a deadline $D$ is the regularised lower incomplete gamma
function

$$P(k, D/S) = \frac{\gamma(k, D/S)}{\Gamma(k)}$$

`Reliability.deadlineSuccess(steps, service, deadline)`. For integer $k$ — which
is what a call depth is — the closed form

$$P(k, x) = 1 - \sum_{n=0}^{k-1} \frac{x^n e^{-x}}{n!}, \qquad x = D/S$$

is evaluated instead, accumulating Poisson terms as $t_n = t_{n-1} \cdot x / n$
from $t_0 = e^{-x}$. Every term stays in $[0, 1]$, no factorial is formed, and
all terms are positive so the sum does not cancel. A single stage is the
exponential CDF, evaluated as $-\mathrm{expm1}(-x)$ so a deadline far shorter
than the service time keeps its significant figures. Non-integer shapes fall
through to the general incomplete gamma, and
`the_closed_form_agrees_with_the_general_incomplete_gamma` holds the two to
within $10^{-9}$ across the domain.

The [`timeout`](./catalogue.md#timeout) behaviour uses the one-stage case
against the observed latency, capping latency per draw at the budget and
converting the tail it cut off into failure and into cancellation:

```yaml
responses:
  latency: min(signal.latency, budget)
  success: signal.success * Reliability.deadlineSuccess(1, max(signal.latency, 1e-6), budget)
```

::: warning Assumption: exponential steps
Exponential service has a coefficient of variation of one, but it is not a
universal conservative bound. Against deterministic service with the same mean,
for example, it overstates success below the mean and understates it above the
mean. Treat `deadlineSuccess` as the result of its stated model, not as a lower
bound. When the latency distribution is known, model `service_time` with that
distribution and read the share of draws under budget instead.
:::

### Quorums

A request answered once $r$ of $n$ independent nodes have replied succeeds when
at least $r$ of them do, which is the upper tail of a binomial count:

$$P(\text{success}) = \sum_{i=r}^{n} \binom{n}{i} p^i (1-p)^{n-i} = I_p(r,\, n-r+1)$$

The right-hand identity is the regularised incomplete beta function, and it is
what is evaluated. The sum forms binomial coefficients that overflow for a group
of a few hundred while the terms they multiply are not yet negligible, whereas
the beta form is stable over the whole domain and accepts a node count that is
not a whole number — which an averaged replica count is not.

The waiting time moves the same way. With $n$ nodes whose response times are
exponential with mean $L$, the time until the $r$th reply is the $r$th order
statistic: a sum of independent exponentials with rates
$n, n-1, \ldots, n-r+1$ by Rényi's representation, and therefore

$$\mathbb{E}[X_{(r)}] = L \sum_{i=n-r+1}^{n} \frac{1}{i} = L\,(H_n - H_{n-r})$$

Harmonic numbers are evaluated as $H_x = \psi(x+1) + \gamma$ through the digamma
function, so a group size that is not a whole number lands on the same smooth
curve as one that is.

The [`quorum`](./catalogue.md#quorum) component reads its group size from the
deployment — `max(out.members.peers, 1)` — takes a strict majority
$r = \lfloor n/2 \rfloor + 1$, and applies both laws. This is the one place in
the catalogue where adding a dependency makes a design *more* reliable and
*faster*.

| $n$ | $r$ | Success at $p = 0.99$ | Wait, in units of $L$ |
| --- | --- | --- | --- |
| 1 | 1 | 0.9900 | 1.000 |
| 3 | 2 | 0.99970 | 0.833 |
| 3 | 3 (all) | 0.9703 | 1.833 |
| 5 | 3 | 0.9999901 | 0.783 |
| 5 | 5 (all) | 0.9510 | 2.283 |

The gap between rows 2 and 3 is the entire argument for a quorum. Waiting for a
majority of three beats a single node on both axes; waiting for all three loses
on both. `a_majority_is_more_available_than_any_of_its_nodes` and
`waiting_for_a_majority_is_faster_than_waiting_for_a_node` hold the
implementation to exactly that, checking the two-of-three case against
$3p^2(1-p) + p^3$ and the wait against $\tfrac13 + \tfrac12$ written out.

What a quorum does **not** do is reduce load. Every node receives every request,
so `issued = arriving × nodes` and the group costs precisely what a fan-out
costs. It buys latency and availability, never throughput.

::: warning Assumption: independent failure
Both quorum results — and both retry results — assume nodes and attempts fail
independently. That is optimistic in the way it always is. A group whose nodes
share a rack, a release, or a poisoned request fails together; attempts against
a saturated dependency fail together. Correlated failure must be expressed as
shared upstream uncertainty (one distribution feeding several components) rather
than left to this arithmetic, which cannot see it.
:::

### Service levels

An objective $o$ is the fraction of eligible operations required to succeed over
a window $T$. The error budget is the number of failures the objective permits
in that window, and burn rate compares observed failure against that allowance
as a multiple:

$$\text{budget} = \lambda\,T\,(1 - o), \qquad \text{burn} = \frac{r}{1 - o}$$

for an observed error ratio $r$. `Slo.errorBudget(rate, objective, window)` and
`Slo.burnRate(observed, objective)`.

A burn rate of one exhausts the budget exactly at the end of the window, two
exhausts it at the halfway point, and below one leaves budget unspent.
Expressing burn as a multiple rather than as a count is what makes a single
alerting threshold work across windows of different lengths.
`burn_rate_is_a_multiple_of_the_permitted_ratio` holds the implementation to it.

A perfect objective is rejected rather than returning an infinity: $o = 1$
leaves no budget to burn, and `a_perfect_objective_has_no_budget_to_burn` pins
that as a diagnostic.

### Forecast scoring

`Dist.klDivergence(estimate, answer)` estimates the Kullback–Leibler divergence

$$D_{\mathrm{KL}}(A \Vert E) = \mathbb{E}_A\!\left[\ln p_A(X) - \ln p_E(X)\right]$$

by Monte Carlo over the runtime's seeded draws, and `Dist.logScore` returns
$-\ln p_E(y)$ for a scalar answer or the divergence above for a distributional
one, subtracting a prior's score where one is supplied.

::: warning Assumption: absolutely continuous densities
The estimate uses the configured draw count, reports no quadrature error bound,
and clamps zero densities to the smallest positive `f64`. It is unsuitable for
singular or mixed measures — a clamped distribution with an atom on its limit is
exactly such a measure — and the returned figure should be read as a comparative
score between forecasts rather than as a calibrated quantity.
:::

---

## Fan-out and tail amplification

The [`aggregator`](./catalogue.md#aggregator) waits for every branch and needs
every branch. Both effects work against the caller at once.

$$\lambda_{\text{branch}} = n \lambda, \qquad
p_{\text{request}} = \prod_{i=1}^{n} p_i, \qquad
W = \max_i W_i + \text{overhead}$$

Caller-facing capacity is the branch ceiling divided by the fan-out, because one
request consumes a branch's capacity $n$ times over:

```yaml
branch_capacity: min(out.branches.capacity, 1e15) / max(branches, 1)
```

Adding a branch is therefore never free, even when the branch is fast and
reliable:

| Branches | Success at $p = 0.999$ each | Downstream demand | Caller capacity |
| --- | --- | --- | --- |
| 1 | 0.9990 | 1× | 1× |
| 5 | 0.9950 | 5× | 1/5 |
| 10 | 0.9900 | 10× | 1/10 |
| 50 | 0.9512 | 50× | 1/50 |

Tail amplification is the second half. If a single branch exceeds 100 ms one
time in a hundred, then a request fanning out to ten independent branches
exceeds it with probability $1 - 0.99^{10} = 9.6\%$ — the service's p99 has
become the request's p90. This is the effect Dean and Barroso named "the tail at
scale", and it is why a fan-out is the wrong shape for a latency-sensitive path
however healthy each branch looks in isolation.

Contrast with a quorum, which fans out identically but stops waiting at $r$ of
$n$. The load is the same; the latency and availability invert.

The [`fan-out`](./catalogue.md#fan-out) behaviour applies the same multiplier to
a relationship without introducing a component, and multiplies `occupancy` as
well as `rate` — a caller waiting on six branches holds six calls open, and a
dependency limited by concurrency rather than throughput would otherwise see a
sixth of the load.

::: warning What Optimist does not compute here
`aggregator.latency` is the **maximum of the branch latency signals per draw**,
not the expected maximum of $n$ random response times. The order statistic is
computed in exactly one place — `Reliability.quorumLatency` — because a quorum
knows how many identical nodes it is waiting on and an aggregator's branches are
different components with different distributions. If a design needs the
distribution of the slowest of $n$ identical branches, represent those branches
explicitly where practical or define a project-local component type with the
appropriate maximum order statistic. The shipped `quorum` always waits for a
strict majority and cannot be configured with $r = n$.
:::

---

## Scaling

Replication is expressed with scale units rather than with per-component replica
counts. A component's properties describe **one replica**; the scale unit says
how many exist and how demand meets them.

| Distribution | Extensive signals | Reading |
| --- | --- | --- |
| `sharded` | divide on entry, gather on exit | Each replica serves its share, as a load-balanced fleet does. |
| `mirrored` | neither divide nor gather | Every replica sees the whole flow, as with writes replicated everywhere. |

A signal is **extensive** if it scales with the size of the system it describes:
`rate`, `cancellation`, `occupancy` and `capacity` are extensive, while
`latency`, `success` and `payload` are intensive and unchanged by how many
replicas there are.

Units nest, and a component's effective replica count is the **product** along
its chain of enclosing units. A component inside ten shards inside three regions
is deployed thirty times; if the regions are mirrored and the shards sharded,
each copy serves a tenth of the demand rather than a thirtieth.

Relationships between members of the same unit stay local — a shard's writer
talks to the one store inside its own shard, not to every shard's. That is what
lets a `quorum` read its group size from `peers` without restating it as a
property that can drift out of step with the scale unit beside it.

Constraints are evaluated **per unit**, which is the question worth asking.
"Does one cell have enough capacity" has an answer an engineer can act on; "does
the fleet have enough capacity in total" hides the cell that is hot while the
average looks fine.

### What Optimist does not model

Scale units divide demand linearly. Optimist implements **neither Amdahl's Law
nor the Universal Scalability Law**, and does not infer contention or coherency
costs from a replica count.

Amdahl's Law bounds speed-up by the serial fraction $\sigma$ of a workload:

$$S(N) = \frac{1}{\sigma + (1 - \sigma)/N}$$

Gunther's Universal Scalability Law adds a coherency term, which makes
throughput fall rather than plateau past a peak:

$$C(N) = \frac{N}{1 + \alpha(N - 1) + \beta N (N - 1)}$$

with $\alpha$ the contention coefficient and $\beta$ the coherency coefficient.

Neither is applied automatically. Doubling a sharded replica count in Optimist
halves the demand each replica sees, exactly, with no cross-replica cost. If a
design has one — a shared lock, a coordination round, a cache that has to be
invalidated everywhere — say so by making `service_time` or `parallelism` a
function of the replica count yourself:

```yaml
scratchpad:
- name: replicas
  expression: '12'
  unit: '1'
- name: contention
  expression: '0.03'
  unit: '1'
  summary: USL alpha — the contention coefficient.
- name: coherency
  expression: '0.0005'
  unit: '1'
  summary: USL beta — cross-replica coordination cost.
- name: idle_service_time
  expression: '0.004'
  unit: s
- name: service_time
  expression: >
    idle_service_time * (1 + contention * (replicas - 1)
      + coherency * replicas * (replicas - 1))
  unit: s
  summary: >
    Service time stretched by the USL terms, so a larger fleet is slower per
    operation rather than linearly faster.
```

Point the component's `service_time` at that quantity and an intervention can
move `replicas` while the coordination cost follows. Writing it down is better
than assuming linearity, and better than assuming a coefficient nobody measured.

::: warning Assumption: linear division
An Optimist model of a fleet at $N$ replicas says nothing about whether that
fleet can be built. It answers "given $N$ replicas each behaving as described,
where does the design bind" — and the phrase doing the work is *each behaving as
described*. Measure the per-replica figures at the fleet size you intend to run,
or express the degradation explicitly as above.
:::

---

## Uncertainty and Jensen's inequality

Queueing delay is **convex** in utilisation. For a convex $f$ and a random $X$,
Jensen's inequality gives

$$\mathbb{E}[f(X)] \geq f(\mathbb{E}[X])$$

so the mean delay under uncertain utilisation strictly exceeds the delay
evaluated at mean utilisation. A model that carries point estimates does not
merely lose the spread of its answer — it reports a **lower** mean than the
truth, and the error grows with the uncertainty and with how close the design
sits to saturation.

The prelude test `uncertain_utilisation_raises_mean_queueing_delay` states this
as a claim about the implementation:

```squiggle
Queue.mm1Wait(0.01, 0.6)                    // delay at the centre
Queue.mm1Wait(0.01, uniform(0.3, 0.9))      // averaged across the spread
```

The second is required to exceed the first by at least 5%. The same convexity
runs through `1/(1-\rho)` everywhere it appears, including the datastore
stretch and the saturation fold, where an uncertain arrival rate straddling
$\lambda^{*}$ produces a bimodal answer that no mean-only model can represent at
all.

This is why every quantity in Optimist is carried as an ensemble of draws rather
than a summary, and why saturation is taken **per draw**:
`saturation_clamps_each_draw_against_capacity` checks that
`min([uniform(50, 150), 100])` has mean 87.5 rather than 100, which comparing
summaries could not produce. See [uncertainty](../guide/uncertainty.md) and
[distributions](../guide/distributions.md) for how to state it.

---

## Fixed points, convergence and multiple steady states

A design with feedback — a retry reading the success rate it is producing, a
pool whose capacity depends on a dependency whose latency depends on that pool —
has no evaluation order. It has a fixed point, and Optimist finds it by
successive relaxation.

Each pass evaluates every component from the current state and moves a fraction
$\eta$ of the way toward the computed value:

$$x_{k+1} = x_k + \eta\,(f(x_k) - x_k)$$

| Setting | Default | Why |
| --- | --- | --- |
| `damping` ($\eta$) | 0.2 | Where each draw *opens*, not a ceiling. A cancelling timeout and the load it relieves form an oscillator that overshoots at 0.5. |
| `tolerance` | $10^{-6}$ | Largest relative movement of any draw treated as settled. |
| `max_iterations` | 1 500 | A retry against a saturated dependency has loop gain just under one and converges steadily but without hurry. |
| `sample_count` | 1 000 | Draws carried through every quantity. |
| `shares` | 4 | Draws divided across workers. Exact, not approximate — each draw settles independently. |

Damping is **per draw** and adaptive. A draw whose movement grows by more than
5% over the previous pass has overshot and halves its stride, to a floor of
0.02; a draw that has contracted for eight consecutive passes doubles its
stride, to a ceiling of 1.0. Because each draw is damped separately, a draw
solved in a share of four reaches the same value it would have reached solved
whole — which is what makes dividing the ensemble free of the answer.

### Two convergence tests

**Per draw.** The solve has settled when no draw moves by more than the
tolerance. That is the right test for a design with one fixed point.

**Distributional.** It is the wrong test for a design with several. Past a fold,
a draw can sit on a branch whose slope is steeper than the damped step can
follow and swap between two values forever. The *ensemble* is perfectly still —
the same draws land on the same two values every pass, only trading places — but
the per-draw test sees a quantity moving by most of its own magnitude.

So when the iterate stops closing — no better than 98% of its best movement over
a window of 128 passes — the ensemble is compared with itself across that window
by **order statistics** rather than by draw. Sorting removes the assignment of
values to draws and leaves the empirical quantile function, which is invariant
under any permutation of the branches. A stationary mixture then reads as no
movement at all, whatever the length of the cycle its draws are going round, and
the solve reports a mixture rather than a failure to converge.
`draws_that_only_traded_places_have_not_moved` states the property.

::: warning Assumption: a settled ensemble has settled
The distributional test cannot distinguish a stationary mixture from a cycle of
the whole ensemble whose period divides the measurement window. The window is
long and unrelated to anything in a model, which makes that a remote coincidence
rather than an impossibility. A design reported as a mixture should be read as
"there is more than one resting state here", which is a fact worth acting on
regardless.
:::

---

## Ranking

Every constraint pairs a demand with the limit it consumes. Because both sides
are carried as sample sets, the ratio is taken per draw and the answer is a
distribution rather than a figure. The share of draws in which demand meets or
exceeds the limit is what constraints are ranked by:

$$P(\text{bind}) = \frac{1}{n}\sum_{i=1}^{n} \mathbb{1}\{d_i \geq l_i\}$$

Ties are broken by mean utilisation, then by component, link and constraint name
so the order is stable across runs. A limit of zero admits no demand at all, so
any demand against it counts as fully saturating rather than undefined.

Ranking by probability rather than by mean utilisation puts the constraint most
exposed to a bad draw at the top. Two constraints at the same average load are
not equally urgent if one of them is far more variable: a constraint at 60% mean
utilisation that exceeds its limit in a fifth of draws is a live problem, and one
at 60% that never exceeds it is not.

Each ranked constraint also reports `utilisation_p90`, `headroom` (mean limit
less mean demand, in the constraint's own units), and `replicas` — the figures
describe one replica, so a constraint that binds does so in each copy.

The engine attaches no meaning to any constraint's name. A limit called `iops`
and one called `concurrency` are ranked by identical arithmetic, which is what
lets a project-local component type introduce a resource nobody anticipated and
still have it reported. See [analysis](../guide/analysis.md) for reading the
output.

---

## References

**Queueing theory**

- John D. C. Little, "A Proof for the Queuing Formula $L = \lambda W$",
  *Operations Research* 9(3), 1961.
  [doi:10.1287/opre.9.3.383](https://doi.org/10.1287/opre.9.3.383)
- Leonard Kleinrock, *Queueing Systems, Volume 1: Theory*, Wiley, 1975 —
  chapters 3 and 4 for M/M/1, M/M/c and the bounded queue.
- J. F. C. Kingman, "The single server queue in heavy traffic",
  *Mathematical Proceedings of the Cambridge Philosophical Society* 57(4), 1961.
  [doi:10.1017/S0305004100036094](https://doi.org/10.1017/S0305004100036094) —
  the $\rho/(1-\rho)$ scaling holds for general arrival and service
  distributions, with variability entering as a separate factor.
- A. K. Erlang, "Solution of some problems in the theory of probabilities of
  significance in automatic telephone exchanges", *Elektroteknikeren* 13, 1917.
- ITU-T Recommendation E.521, *Calculation of load capacity in a local telephone
  network* — the numerically stable Erlang recursions.

**Special functions and order statistics**

- Milton Abramowitz and Irene Stegun, *Handbook of Mathematical Functions*, 1964
  — section 6.5 for the incomplete gamma function (equation 6.5.13 for the
  integer-shape closed form) and equation 26.5.24 for the binomial–beta identity.
- Alfréd Rényi, "On the theory of order statistics", *Acta Mathematica Hungarica*
  4, 1953 — the exponential order statistic representation used for quorum
  latency.

**Scaling**

- Gene Amdahl, "Validity of the single processor approach to achieving large
  scale computing capabilities", *AFIPS Conference Proceedings* 30, 1967.
- Neil Gunther, *Guerrilla Capacity Planning*, Springer, 2007, and the
  [Universal Scalability Law](http://www.perfdynamics.com/Manifesto/USLscalability.html).
  Not implemented by Optimist; see [Scaling](#scaling).

**Reliability and failure modes**

- Jeffrey Dean and Luiz André Barroso, "The Tail at Scale",
  *Communications of the ACM* 56(2), 2013.
  [cacm.acm.org](https://cacm.acm.org/research/the-tail-at-scale/)
- Nathan Bronson, Abutalib Aghayev, Aleksey Charapko and Timothy Zhu,
  "Metastable Failures in Distributed Systems", *HotOS XVIII*, 2021.
  [sigops.org](https://sigops.org/s/conferences/hotos/2021/papers/hotos21-s11-bronson.pdf)
- Marc Brooker, ["Exponential Backoff And Jitter"](https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter/),
  AWS Architecture Blog, 2015, and
  ["Metastability and Distributed Systems"](https://brooker.co.za/blog/2021/05/24/metastable.html), 2021.
- Google, *Site Reliability Engineering*, O'Reilly, 2016 — chapters 3 and 4 on
  error budgets — and *The Site Reliability Workbook*, 2018 — chapter 5 on
  [burn rate alerting](https://sre.google/workbook/alerting-on-slos/).

**Estimation practice**

- Simon Eskildsen, [napkin-math](https://github.com/sirupsen/napkin-math) —
  the base rates worth committing to memory before reaching for a model.

---

Continue with the [shipped catalogue](./catalogue.md) for the components these
laws are wired into, [modelling](../guide/modelling.md) for building a design,
or the [language reference](../guide/language.md) for writing the expressions
yourself.
