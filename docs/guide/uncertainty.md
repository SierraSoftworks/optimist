# Uncertainty and statistics

Optimist treats uncertainty as a modelled quantity, not a generic confidence percentage. Distribution support, units, shared references, dependence, random seeds, and convergence diagnostics are explicit.

## Primitive distributions

| Distribution | Support | Typical use |
| --- | --- | --- |
| Point | One finite value | A known constant or explicit no-uncertainty assumption. |
| Normal | $(-\infty,\infty)$ | Additive, unbounded uncertainty. |
| LogNormal | $(0,\infty)$ | Positive multiplicative quantities such as time or cost. |
| Beta | $[0,1]$ | Probabilities and normalized states. |
| Scaled Beta | $[l,u]$ | Bounded quantities such as signed influence. |

Typed estimate dimensions reject incompatible support. A probability cannot use a Normal distribution because its complete support extends outside $[0,1]$. Money and duration reject distributions which permit negative values.

## Fermi decomposition

Use a Fermi formula when a difficult estimate can be decomposed into quantities which are easier to elicit.

For monthly delivery effort:

$$
T_{month} = N_{deployments} \times T_{per\ deployment}
$$

The [Fermi example](../examples/#fermi-delivery-time-estimate) models deployment count with a Scaled Beta and time per deployment with a LogNormal distribution. Runtime unit algebra proves the result is measured in minutes.

```sh
cargo run --example fermi_delivery_time
```

Formula operations include sums, products, ratios, integer powers, bounded transforms, and references. References are memoized once per Monte Carlo draw, so repeated use of one uncertain address does not accidentally assume independent copies.

### Workbench elicitation

The workbench accepts compact central estimates such as `1.5M`. By default, each positive variable uses a LogNormal whose median is the entered estimate and whose central 90% interval spans one order of magnitude in either direction. If $m$ is the estimate, then:

$$
\mu=\ln m, \qquad \sigma=\frac{\ln 10}{\Phi^{-1}(0.95)}.
$$

This is deliberately broad. Expanding a variable enables a custom three-point Beta-PERT component. For low $a$, most-likely $m$, and high $b$, it uses a Scaled Beta on $[a,b]$ with:

$$
\alpha = 1 + 4\frac{m-a}{b-a}, \qquad
\beta = 1 + 4\frac{b-m}{b-a}.
$$

Variables accept human unit expressions such as `people/household`, `piano*days/tuning`, and `(pianos/day)^2`. Optimist singularizes simple English plurals, composes exponents through the equation, and compares the result with the goal unit. Equations support `+`, `-`, `*`, `/`, integer powers, numeric constants, and parentheses. Addition and subtraction require equal units.

The live program also uses Squiggle's unit type annotations. Optimist converts each parsed canonical unit to declarations such as `rate :: item/day = ...` and annotates both the unbounded equation result and its support-bounded preview. Squiggle therefore reports dimensional conflicts while the equation is edited. The canonical Rust `Formula` remains authoritative for persistence because Squiggle currently treats exponentiation and most built-in functions as accepting or returning any unit type, whereas Optimist validates every formula operation and target unit.

That expression surface is versioned as `optimist_squiggle_v1` and is intentionally compatible with the corresponding arithmetic subset of [Squiggle](https://www.squiggle-language.com/). The workbench translates variable distributions into Squiggle, evaluates the expression after a short debounce with a fixed seed, and immediately shows its expected value, standard deviation, median, inner 50% band, and central 90% interval. It also evaluates the unbounded expression before applying probability or signed-state clamps and reports how much mass violates the estimate slot's support. Non-negative slots are not silently clamped: negative mass remains visible and prevents eventual server adoption. This is a prior-predictive review aid: it helps expose surprising implications while inputs are being edited, but it is not the persisted result or final validation.

For example, the central piano-tuning arithmetic is:

$$
1{,}500{,}000 / 3 / 20 / 180 \times 1 = 138.889.
$$

With units exactly `people`, `people/household`, `households/piano`, `days/tuning`, and `pianos/tuning`, dimensional analysis produces `piano^2/day`, not `piano/day`. The assistant reports the unresolved `piano` dimension. One coherent correction is to describe the tuning interval as `piano*days/tuning`: each tuning event is required per piano per 180 days. The same equation then resolves to `piano/day`.

Optimist samples the resolved equation and recommends an effective primitive family compatible with the requested support by matching sampled mean and variance. A stored estimate has exactly one active source: a directly authored distribution or a persisted Fermi equation. Fermi estimates retain their source-language version, equation, variables, canonical formula, sampling controls, assessment diagnostics, interval, and effective distribution; existing analyses consume that effective distribution. The Rust API reassesses the canonical unit-checked formula and remains authoritative rather than trusting the browser's Squiggle preview. Saving a direct distribution later replaces the Fermi source rather than layering two competing priors. Canonical project archives retain the complete embedded source; definitions written before the language marker was introduced migrate to `optimist_squiggle_v1` during deserialization.

Native quantities derive their target unit and support from the owning metric. Real quantities moment-match to Normal, non-negative quantities to LogNormal, and arbitrary bounded intervals to Scaled Beta on the declared native bounds. Legacy metrics without canonical unit terms remain usable for observations and direct estimates but cannot persist a typed Fermi source until their quantity definition is upgraded.

The hand-calculated `138.889` is the equation evaluated at entered central values; it is not generally equal to the Monte Carlo expectation of broad products and ratios. Five independent order-of-magnitude priors create a very wide distribution and may hit the sample limit. Refine variables with evidence rather than interpreting that broad default as calibrated confidence. The reported 90% interval belongs to the moment-matched recommendation and does not preserve multimodality or exact tail shape.

## Metric calibration

A `measures` relationship may explicitly map readings in a metric's unit to the measured factor or outcome's normalized state. For linear anchors $x_0$ at state zero and $x_1$ at state one:

$$
s(x)=\operatorname{clamp}\left(\frac{x-x_0}{x_1-x_0},0,1\right).
$$

Lower-is-better metrics use $x_0>x_1$. Target-range metrics use two linear ramps from outer state-zero anchors to an ideal state-one interval. Values outside the outer anchors clamp to zero.

Calibration makes interpretation visible; it is not an automatic statistical update. Observations remain immutable edge-local records, and state estimates change only when a caller explicitly adopts a calibrated reading or performs a separately justified Bayesian update.

## Monte Carlo convergence

Sampling uses a pinned ChaCha20 stream. Given the same model, seed, configuration, and dependency versions, results are bit-reproducible.

Sampling stops after the minimum valid sample count once every output satisfies:

$$
SE(\bar X) \le a + r|\bar X|
$$

where $a$ is the absolute tolerance and $r$ is the relative tolerance. Reports keep:

- attempted and valid sample counts,
- invalid draw counts by numerical cause,
- convergence status,
- sample mean and variance,
- Monte Carlo standard errors,
- the seed and complete stopping criterion.

Monte Carlo standard error measures numerical sampling noise. It is not the uncertainty of the model itself.

## Bayesian updates

Optimist currently implements two conjugate updates:

### Beta-Binomial

For prior $p \sim \operatorname{Beta}(\alpha,\beta)$ and $s$ successes in $n$ conditionally independent trials:

$$
p \mid s,n \sim \operatorname{Beta}(\alpha+s,\beta+n-s)
$$

```sh
cargo run --example bayesian_delivery_success
```

The likelihood assumes exchangeable Bernoulli trials with one stable success probability. It does not model overdispersion, censoring, changing rates, or correlated trials.

### Normal-Normal

For an unknown mean with a Normal prior and observations with known variance, Optimist combines prior and data precision analytically. The observation variance is not inferred.

## Quantile elicitation

Normal and LogNormal priors can be fitted from elicited quantiles. Optimist retains the supplied quantiles and returns residual diagnostics rather than discarding disagreement between the elicitation and fitted family.

Use Normal fitting for additive uncertainty in value space and LogNormal fitting for positive multiplicative uncertainty in log space.

## Dependence

Do not multiply confidence scores or silently assume independent estimates. A project dependence document can group marginals under a Gaussian copula with a rank or latent correlation matrix.

Optimist validates:

- same-project unique member addresses,
- non-overlapping groups,
- finite coefficients in $[-1,1]$,
- symmetry and unit diagonal,
- positive semidefiniteness with a documented numerical tolerance,
- literal marginal availability for inverse-CDF sampling.

Point masses and discrete ties may reduce observed rank correlation. Singular positive-semidefinite matrices are supported.

## Review checklist

Before relying on a result, ask:

1. Does each distribution family match the quantity's support and error mechanism?
2. Are units correct through every formula?
3. Are repeated references intentionally shared?
4. Are residual correlations modelled where independence is implausible?
5. Did sampling converge, and is Monte Carlo error small relative to model uncertainty?
6. Are Bayesian likelihood assumptions justified by the observations?
7. Can another person reproduce the result from the retained seed and diagnostics?
