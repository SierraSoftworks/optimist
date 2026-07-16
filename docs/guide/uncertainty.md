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
