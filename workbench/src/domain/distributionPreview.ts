import type { Distribution } from '../api/types'

export interface DensityPoint {
  value: number
  density: number
}

export interface DistributionPreviewModel {
  family: string
  support: string
  summary: string
  domain: [number, number]
  density: DensityPoint[]
  marker: number | null
}

const sampleCount = 121

function finite(value: number | undefined, fallback: number) {
  return value !== undefined && Number.isFinite(value) ? value : fallback
}

function positive(value: number | undefined, fallback: number) {
  const candidate = finite(value, fallback)
  return candidate > 0 ? candidate : fallback
}

function format(value: number) {
  const magnitude = Math.abs(value)
  if (magnitude !== 0 && (magnitude >= 10_000 || magnitude < 0.001)) return value.toExponential(2)
  return Number(value.toPrecision(4)).toString()
}

function sample(
  domain: [number, number],
  logDensity: (value: number, position: number) => number,
) {
  const logs = Array.from({ length: sampleCount }, (_, index) => {
    const position = index / (sampleCount - 1)
    const value = domain[0] + (domain[1] - domain[0]) * position
    return { value, logDensity: logDensity(value, position) }
  })
  const maximum = Math.max(...logs.map((point) => point.logDensity).filter(Number.isFinite))
  return logs.map((point) => ({
    value: point.value,
    density: Number.isFinite(point.logDensity) ? Math.exp(point.logDensity - maximum) : 0,
  }))
}

/**
 * Produces a normalized visual model for Optimist's primitive distributions.
 *
 * Equations and parameterization match the Rust domain types:
 * - Normal: log f(x) = -0.5 ((x - mu) / sigma)^2, sampled on mu +/- 4 sigma.
 * - LogNormal: log f(x) = -log(x) - 0.5 ((log(x) - mu) / sigma)^2,
 *   sampled from exp(mu - 4 sigma) to exp(mu + 4 sigma).
 * - Beta and Scaled Beta: log f(t) = (alpha - 1) log(t) + (beta - 1) log(1 - t),
 *   where t is on (0, 1); Scaled Beta maps t onto [lower, upper].
 * - Point is represented as a marker because it has no finite density curve.
 *
 * The curves are relative densities, normalized to a maximum height of one. The
 * normalizing constants therefore cancel. Sampling is deterministic at 121 evenly
 * spaced positions; Beta endpoints are inset by 1e-4 to represent integrable poles
 * without infinities. This is a visual explanation, not a numerical integrator,
 * quantile calculator, or Monte Carlo approximation. Invalid in-progress form
 * parameters are replaced with conservative finite defaults so the preview remains
 * renderable while native input validation prevents submission.
 *
 * References: NIST/SEMATECH e-Handbook of Statistical Methods, sections 1.3.6.6
 * (probability distributions), 1.3.6.6.1 (Normal), 1.3.6.6.9 (Lognormal), and
 * 1.3.6.6.17 (Beta).
 */
export function distributionPreview(
  distribution: Distribution,
  domainOverride?: [number, number],
): DistributionPreviewModel {
  switch (distribution.type) {
    case 'point': {
      const value = finite(distribution.value, 0)
      const span = Math.max(Math.abs(value) * 0.2, 1)
      return {
        family: 'Point',
        support: `Exactly ${format(value)}`,
        summary: `Every model run uses ${format(value)}. No uncertainty is represented.`,
        domain: domainOverride ?? [value - span, value + span],
        density: [],
        marker: value,
      }
    }
    case 'normal': {
      const mean = finite(distribution.mean, 0)
      const standardDeviation = positive(distribution.standard_deviation, 1)
      const domain: [number, number] = [
        mean - 4 * standardDeviation,
        mean + 4 * standardDeviation,
      ]
      return {
        family: 'Normal',
        support: 'Any real value',
        summary: `Centered at ${format(mean)}; roughly 95% falls within ${format(mean - 2 * standardDeviation)} to ${format(mean + 2 * standardDeviation)}.`,
        domain,
        density: sample(domain, (value) => -0.5 * ((value - mean) / standardDeviation) ** 2),
        marker: null,
      }
    }
    case 'log_normal': {
      const location = finite(distribution.location, 0)
      const scale = positive(distribution.scale, 1)
      const domain: [number, number] = [
        Math.exp(location - 4 * scale),
        Math.exp(location + 4 * scale),
      ]
      const median = Math.exp(location)
      return {
        family: 'LogNormal',
        support: 'Positive values only',
        summary: `Median ${format(median)} with a long upper tail; one log-scale step multiplies by about ${format(Math.exp(scale))}.`,
        domain,
        density: sample(domain, (value) =>
          -Math.log(value) - 0.5 * ((Math.log(value) - location) / scale) ** 2,
        ),
        marker: null,
      }
    }
    case 'beta': {
      const alpha = positive(distribution.alpha, 2)
      const beta = positive(distribution.beta, 2)
      const domain: [number, number] = [0, 1]
      const mean = alpha / (alpha + beta)
      return {
        family: 'Beta',
        support: 'Between 0 and 1',
        summary: `Mean ${format(mean)}. Alpha pulls weight upward; beta pulls it downward; larger totals mean tighter confidence.`,
        domain,
        density: sample(domain, (_value, position) => {
          const inset = Math.min(1 - 1e-4, Math.max(1e-4, position))
          return (alpha - 1) * Math.log(inset) + (beta - 1) * Math.log1p(-inset)
        }),
        marker: null,
      }
    }
    case 'scaled_beta': {
      const alpha = positive(distribution.alpha, 2)
      const beta = positive(distribution.beta, 2)
      const lower = finite(distribution.lower, 0)
      const proposedUpper = finite(distribution.upper, 1)
      const upper = proposedUpper > lower ? proposedUpper : lower + 1
      const domain: [number, number] = [lower, upper]
      const mean = lower + (upper - lower) * alpha / (alpha + beta)
      return {
        family: 'Scaled Beta',
        support: `${format(lower)} to ${format(upper)}`,
        summary: `Mean ${format(mean)} inside hard bounds. Alpha pulls toward the upper bound; beta pulls toward the lower bound.`,
        domain,
        density: sample(domain, (_value, position) => {
          const inset = Math.min(1 - 1e-4, Math.max(1e-4, position))
          return (alpha - 1) * Math.log(inset) + (beta - 1) * Math.log1p(-inset)
        }),
        marker: null,
      }
    }
  }
}
