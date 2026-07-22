import type { Env, SqError, SqValue } from '@quri/squiggle-lang'
import { fermiSupportBounds, type FermiComponentDraft, type FermiSupport } from './fermiBuilder'
import { formatSquiggleUnitAnnotation, parseUnitExpression } from './unitExpression'

const previewSamples = 4_000
let runtime: Promise<typeof import('@quri/squiggle-lang')> | null = null

export interface SquigglePreview {
  mean: number
  standardDeviation: number
  p05: number
  p25: number
  p50: number
  p75: number
  p95: number
  supportViolationProbability: number
  samples: number
  executionMilliseconds: number
}

export class SquigglePreviewError extends Error {
  readonly line: number | null
  readonly column: number | null

  constructor(message: string, line: number | null = null, column: number | null = null) {
    super(message)
    this.name = 'SquigglePreviewError'
    this.line = line
    this.column = column
  }
}

export async function evaluateSquigglePreview(
  equation: string,
  components: FermiComponentDraft[],
  support: FermiSupport,
  expectedUnit: Record<string, number>,
): Promise<SquigglePreview> {
  const { defaultEnvironment, run } = await (runtime ??= import('@quri/squiggle-lang'))
  const environment: Env = {
    ...defaultEnvironment,
    sampleCount: previewSamples,
    xyPointLength: 200,
    seed: 'optimist-live-preview-v1',
  }
  const prelude = components.map(variableSource)
  const expression = boundedExpression(equation, support)
  const resultUnit = formatSquiggleUnitAnnotation(expectedUnit)
  const output = await run(
    [
      ...prelude,
      `optimist_unbounded :: ${resultUnit} = (${equation})`,
      `optimist_bounded :: ${resultUnit} = ${expression}`,
      '{ unbounded: optimist_unbounded, bounded: optimist_bounded }',
    ].join('\n'),
    { environment },
  )
  if (!output.result.ok) throw previewError(output.result.value.errors[0]!, prelude.length)

  const result = output.result.value.result
  if (result.tag !== 'Dict') {
    throw new SquigglePreviewError('The Squiggle preview did not produce its expected result set.')
  }
  const unbounded = result.value.get('unbounded')
  const bounded = result.value.get('bounded')
  if (!unbounded || !bounded) {
    throw new SquigglePreviewError('The Squiggle preview omitted an expected result.')
  }
  const summary = summarize(bounded, output.environment)
  return {
    ...summary,
    supportViolationProbability: supportViolation(unbounded, output.environment, support),
    samples: previewSamples,
    executionMilliseconds: output.executionTime,
  }
}

function summarize(result: SqValue, environment: Env) {
  if (result.tag === 'Number') {
    return {
      mean: result.value,
      standardDeviation: 0,
      p05: result.value,
      p25: result.value,
      p50: result.value,
      p75: result.value,
      p95: result.value,
    }
  }
  if (result.tag !== 'Dist') {
    throw new SquigglePreviewError('The Squiggle expression must produce a number or distribution.')
  }

  const distribution = result.value
  const standardDeviation = distribution.stdev(environment)
  const p05 = distribution.inv(environment, 0.05)
  const p25 = distribution.inv(environment, 0.25)
  const p50 = distribution.inv(environment, 0.5)
  const p75 = distribution.inv(environment, 0.75)
  const p95 = distribution.inv(environment, 0.95)
  if (!standardDeviation.ok || !p05.ok || !p25.ok || !p50.ok || !p75.ok || !p95.ok) {
    throw new SquigglePreviewError('Squiggle could not summarize the resulting distribution.')
  }
  return {
    mean: distribution.mean(environment),
    standardDeviation: standardDeviation.value,
    p05: p05.value,
    p25: p25.value,
    p50: p50.value,
    p75: p75.value,
    p95: p95.value,
  }
}

function supportViolation(result: SqValue, environment: Env, support: FermiSupport) {
  if (support === 'real') return 0
  if (result.tag === 'Number') {
    const bounds = fermiSupportBounds(support)
    const valid = support === 'non_negative'
      ? result.value >= 0
      : bounds
        ? result.value >= bounds[0] && result.value <= bounds[1]
        : true
    return valid ? 0 : 1
  }
  if (result.tag !== 'Dist') {
    throw new SquigglePreviewError('The Squiggle expression must produce a number or distribution.')
  }
  const bounds = fermiSupportBounds(support)
  const lowerBound = support === 'non_negative' ? 0 : bounds?.[0]
  const upperBound = bounds?.[1]
  if (lowerBound === undefined) return 0
  const lower = result.value.cdf(environment, lowerBound)
  const upper = upperBound === undefined ? null : result.value.cdf(environment, upperBound)
  if (!lower.ok || (upper && !upper.ok)) {
    throw new SquigglePreviewError('Squiggle could not check the result against its required support.')
  }
  return Math.max(0, Math.min(1, lower.value + (upper ? 1 - upper.value : 0)))
}

function variableSource(component: FermiComponentDraft) {
  const name = component.name.trim()
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) {
    throw new SquigglePreviewError(`Variable ${JSON.stringify(name)} must use letters, digits, and underscores.`)
  }
  const annotation = formatSquiggleUnitAnnotation(parseUnitExpression(component.unit))
  if ((component.mode ?? 'order_of_magnitude') === 'order_of_magnitude') {
    if (!Number.isFinite(component.likely) || component.likely <= 0) {
      throw new SquigglePreviewError(`Variable ${name} requires a positive finite estimate.`)
    }
    return `${name} :: ${annotation} = lognormal(${Math.log(component.likely)}, ${Math.log(10) / 1.6448536269514722})`
  }
  if (![component.low, component.likely, component.high].every(Number.isFinite) || component.low > component.likely || component.likely > component.high) {
    throw new SquigglePreviewError(`Variable ${name} must satisfy low <= likely <= high.`)
  }
  if (component.low === component.high) return `${name} :: ${annotation} = ${component.low}`
  const width = component.high - component.low
  const alpha = 1 + 4 * (component.likely - component.low) / width
  const beta = 1 + 4 * (component.high - component.likely) / width
  return `${name} :: ${annotation} = beta(${alpha}, ${beta}) * ${width} + ${component.low}`
}

function boundedExpression(equation: string, support: FermiSupport) {
  const bounds = fermiSupportBounds(support)
  if (bounds) return `min(max((${equation}), ${bounds[0]}), ${bounds[1]})`
  return `(${equation})`
}

function previewError(error: SqError, preludeLines: number) {
  if (error.tag === 'other') return new SquigglePreviewError(error.toString())
  const location = error.location()
  if (!location) return new SquigglePreviewError(error.toString())
  const equationLine = location.start.line - preludeLines
  return new SquigglePreviewError(
    error.toString(),
    equationLine > 0 ? equationLine : null,
    location.start.column,
  )
}