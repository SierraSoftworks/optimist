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
| Empirical | Retained finite draws | Arbitrary Squiggle results used by downstream simulation. |

Typed estimate dimensions reject incompatible support. Primitive distributions can be checked from their complete analytical support. Empirical distributions are checked from their retained draws, which is a finite prior-predictive check rather than proof that every possible tail value is valid.

## Squiggle estimates

An estimate may retain direct [Squiggle](https://www.squiggle-language.com/) source whose final expression is a finite number or sampleable distribution. The same source form covers primitive families, transformed distributions, mixtures, decomposition, and simulation-based constructions:

```squiggle
base = lognormal({p5: 4, p95: 10})
interruptions = mixture([pointMass(0), gamma(3, 1)], [0.7, 0.3])
base + interruptions
```

The workbench sends source to Optimist after a short debounce. Rust is the only evaluator: it applies source and step bounds, lints the program, checks the result unit, evaluates with a fixed seed, and returns the family, mean, variance, median, and central 90% interval. The browser neither loads a second runtime nor submits a result distribution.

Each definition retains:

- the authored source,
- the deterministic seed,
- 256 to 4,096 effective draws, with 2,048 as the workbench default,
- the canonical target unit derived from the owning estimate slot.

Optimist wraps the calculation in a Squiggle unit annotation. A duration result is evaluated as if it were assigned to `optimist_result :: duration`; a native metric uses its canonical unit terms. Source annotations can make intermediate assumptions reviewable too:

```squiggle
deployments :: item/month = poisson(20)
effort :: hour/item = lognormal({p5: 0.5, p95: 3})
deployments * effort
```

For a numeric result, Optimist persists a point distribution. For a distribution result, it persists deterministic empirical draws so downstream causal and scenario analysis can sample rich, multimodal, truncated, or transformed results without forcing them into a Normal, LogNormal, or Beta approximation. The backend also persists its assessment. On load, deterministic reevaluation must reproduce both assessment and effective distribution; disagreement is an integrity error.

Probability, signed, non-negative, and bounded slots validate the effective draws before persistence. This catches violations represented in the retained sample, but cannot prove that an unbounded symbolic tail has zero mass outside the target support. Authors should use bounded families or explicit truncation when support is part of the quantity definition, and review tail behavior rather than treating one finite sample as a theorem.

One estimate has one active authoring source. New workbench saves use Squiggle. Legacy direct-distribution and Fermi sources remain deserializable for replay and archives; opening them translates their effective distribution into editable Squiggle, and the next save replaces the legacy source. Existing public Fermi and primitive commands remain compatibility APIs rather than parallel workbench editors.

### Decomposition

When a difficult estimate can be decomposed into easier quantities, write those assumptions directly in Squiggle. For monthly delivery effort:

$$
T_{month} = N_{deployments} \times T_{per\ deployment}
$$

The legacy [Fermi example](../examples/#fermi-delivery-time-estimate) demonstrates the same statistical idea using the typed Rust `Formula` API. Formula references are memoized once per Monte Carlo draw, so repeated use of one uncertain address does not accidentally assume independent copies.

```sh
cargo run --example fermi_delivery_time
```

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
