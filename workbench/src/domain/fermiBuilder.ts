import type { FermiSupport, Formula, Unit } from '../api/types'
import { parseUnitExpression } from './unitExpression'

export type FermiOperation = 'sum' | 'product' | 'ratio'
export type { FermiSupport }

export function fermiSupportBounds(support: FermiSupport): [number, number] | null {
  if (support === 'probability') return [0, 1]
  if (support === 'signed') return [-1, 1]
  if (typeof support === 'object') return [support.bounded.lower, support.bounded.upper]
  return null
}

export interface FermiComponentDraft {
  name: string
  low: number
  likely: number
  high: number
  unit: string
  mode?: 'order_of_magnitude' | 'pert'
}

export function buildFermiFormula(
  operation: FermiOperation,
  components: FermiComponentDraft[],
  support: FermiSupport,
): Formula {
  if (components.length < 2) throw new Error('Add at least two components.')
  if (operation === 'ratio' && components.length !== 2) {
    throw new Error('A ratio requires exactly a numerator and denominator.')
  }
  const terms = components.map(componentFormula)
  const composed: Formula = operation === 'sum'
    ? { type: 'sum', terms }
    : operation === 'product'
      ? { type: 'product', factors: terms }
      : { type: 'ratio', numerator: terms[0]!, denominator: terms[1]! }
  const bounds = fermiSupportBounds(support)
  return bounds
    ? { type: 'bounded', input: composed, lower: bounds[0], upper: bounds[1] }
    : composed
}

export function fermiProvenance(
  operation: FermiOperation,
  components: FermiComponentDraft[],
  samples: number,
) {
  const expression = components
    .map((component) => `${component.name.trim() || 'component'} [${component.low}, ${component.likely}, ${component.high}]${component.unit.trim() ? ` ${component.unit.trim()}` : ''}`)
    .join(operation === 'sum' ? ' + ' : operation === 'product' ? ' × ' : ' ÷ ')
  return `Fermi ${operation}: ${expression}; ${samples} deterministic Monte Carlo samples.`
}

export function componentFormula(component: FermiComponentDraft): Formula {
  if (![component.low, component.likely, component.high].every(Number.isFinite)) {
    throw new Error('Component estimates must be finite numbers.')
  }
  if (component.low > component.likely || component.likely > component.high) {
    throw new Error('Each component must satisfy low ≤ likely ≤ high.')
  }
  const unit = parseUnit(component.unit)
  if ((component.mode ?? 'pert') === 'order_of_magnitude') {
    if (component.likely <= 0) {
      throw new Error('Order-of-magnitude estimates must be greater than zero.')
    }
    return {
      type: 'literal',
      distribution: {
        type: 'log_normal',
        location: Math.log(component.likely),
        scale: Math.log(10) / 1.6448536269514722,
      },
      unit,
    }
  }
  if (component.low === component.high) {
    return { type: 'literal', distribution: { type: 'point', value: component.low }, unit }
  }
  const width = component.high - component.low
  return {
    type: 'literal',
    distribution: {
      type: 'scaled_beta',
      alpha: 1 + 4 * (component.likely - component.low) / width,
      beta: 1 + 4 * (component.high - component.likely) / width,
      lower: component.low,
      upper: component.high,
    },
    unit,
  }
}

function parseUnit(value: string): Unit {
  return parseUnitExpression(value)
}