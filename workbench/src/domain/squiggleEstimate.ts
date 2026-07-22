import type {
  EstimateSourceInput,
  EstimateSupport,
  SquiggleEstimateDefinition,
  Unit,
} from '../api/types'

export function squiggleDefinition(
  source: string,
  targetUnit: Unit,
): SquiggleEstimateDefinition {
  return {
    source,
    seed: 42,
    sample_count: 2_048,
    target_unit: targetUnit,
  }
}

export function defaultSquiggleDefinition(
  support: EstimateSupport,
  targetUnit: Unit,
): SquiggleEstimateDefinition {
  return squiggleDefinition(defaultSquiggleSource(support), targetUnit)
}

export function squiggleSourceInput(
  source: string,
  targetUnit: Unit,
): EstimateSourceInput {
  return { type: 'squiggle', definition: squiggleDefinition(source, targetUnit) }
}

export function defaultSquiggleSourceInput(
  support: EstimateSupport,
  targetUnit: Unit,
): EstimateSourceInput {
  return { type: 'squiggle', definition: defaultSquiggleDefinition(support, targetUnit) }
}

function defaultSquiggleSource(support: EstimateSupport) {
  if (support === 'probability') return 'beta(2, 2)'
  if (support === 'signed') return 'beta(2, 2) * 2 - 1'
  if (support === 'non_negative') return 'lognormal(0, 0.5)'
  if (typeof support === 'object') {
    const { lower, upper } = support.bounded
    return `beta(2, 2) * ${upper - lower} + ${lower}`
  }
  return 'normal(0, 1)'
}
