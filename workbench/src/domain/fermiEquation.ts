import type { Formula, Unit } from '../api/types'
import { componentFormula, type FermiComponentDraft, type FermiSupport } from './fermiBuilder'
import { formatHumanNumber } from './humanNumber'
import { divideUnits, multiplyUnits, powerUnit, unitsEqual } from './unitExpression'

interface Value {
  formula: Formula
  unit: Unit
  central: number
}

type Token =
  | { type: 'name'; value: string }
  | { type: 'number'; value: number }
  | { type: 'operator'; value: '+' | '-' | '*' | '/' | '^' | '(' | ')' }
  | { type: 'end' }

export class FermiEquationError extends Error {
  readonly variables: string[]

  constructor(message: string, variables: string[] = []) {
    super(message)
    this.name = 'FermiEquationError'
    this.variables = variables
  }
}

export function compileFermiEquation(
  source: string,
  components: FermiComponentDraft[],
  support: FermiSupport,
) {
  const variables = new Map<string, Value>()
  for (const component of components) {
    const name = component.name.trim()
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) {
      throw new FermiEquationError(`Variable ${JSON.stringify(name)} must use letters, digits, and underscores.`, [name])
    }
    if (variables.has(name)) throw new FermiEquationError(`Variable ${name} is defined more than once.`, [name])
    let formula: Formula
    try {
      formula = componentFormula(component)
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : 'Variable input is invalid.'
      throw new FermiEquationError(`Variable ${name}: ${message}`, [name])
    }
    if (formula.type !== 'literal') throw new Error('component formulas are literals')
    variables.set(name, { formula, unit: formula.unit, central: component.likely })
  }
  const parser = new EquationParser(tokenize(source), variables)
  const value = parser.parse()
  const formula = support === 'probability'
    ? { type: 'bounded' as const, input: value.formula, lower: 0, upper: 1 }
    : support === 'signed'
      ? { type: 'bounded' as const, input: value.formula, lower: -1, upper: 1 }
      : value.formula
  return { formula, unit: value.unit, central: value.central, referencedVariables: parser.referencedVariables }
}

export function fermiEquationProvenance(
  equation: string,
  components: FermiComponentDraft[],
  samples: number,
) {
  const variables = components.map((component) => {
    const mode = component.mode ?? 'order_of_magnitude'
    const uncertainty = mode === 'order_of_magnitude'
      ? `90% interval ${formatHumanNumber(component.likely / 10)}..${formatHumanNumber(component.likely * 10)}`
      : `PERT ${component.low}..${component.likely}..${component.high}`
    return `${component.name}=${formatHumanNumber(component.likely)} ${component.unit || 'dimensionless'} (${uncertainty})`
  }).join('; ')
  return `Fermi equation: ${equation.trim()}; ${variables}; ${samples} deterministic Monte Carlo samples.`
}

class EquationParser {
  private index = 0
  readonly referencedVariables = new Set<string>()
  private readonly tokens: Token[]
  private readonly variables: Map<string, Value>

  constructor(tokens: Token[], variables: Map<string, Value>) {
    this.tokens = tokens
    this.variables = variables
  }

  parse() {
    const value = this.sum()
    if (this.peek().type !== 'end') throw this.error('Unexpected token')
    return value
  }

  private sum(): Value {
    let left = this.product()
    while (this.operator('+') || this.operator('-')) {
      const operation = this.consume() as Extract<Token, { type: 'operator' }>
      let right = this.product()
      if (!unitsEqual(left.unit, right.unit)) {
        throw this.error('Addition and subtraction require matching units')
      }
      if (operation.value === '-') {
        right = {
          formula: {
            type: 'product',
            factors: [constant(-1), right.formula],
          },
          unit: right.unit,
          central: -right.central,
        }
      }
      left = {
        formula: { type: 'sum', terms: [left.formula, right.formula] },
        unit: left.unit,
        central: left.central + right.central,
      }
    }
    return left
  }

  private product(): Value {
    let left = this.power()
    while (this.operator('*') || this.operator('/')) {
      const operation = this.consume() as Extract<Token, { type: 'operator' }>
      const right = this.power()
      if (operation.value === '/' && right.central === 0) throw this.error('The central denominator is zero')
      left = operation.value === '*'
        ? {
            formula: { type: 'product', factors: [left.formula, right.formula] },
            unit: multiplyUnits(left.unit, right.unit),
            central: left.central * right.central,
          }
        : {
            formula: { type: 'ratio', numerator: left.formula, denominator: right.formula },
            unit: divideUnits(left.unit, right.unit),
            central: left.central / right.central,
          }
    }
    return left
  }

  private power(): Value {
    const value = this.primary()
    if (!this.operator('^')) return value
    this.consume()
    let sign = 1
    if (this.operator('-')) {
      this.consume()
      sign = -1
    }
    const exponent = this.consume()
    if (exponent.type !== 'number' || !Number.isInteger(exponent.value)) {
      throw this.error('Powers must be integer constants')
    }
    return {
      formula: { type: 'power', base: value.formula, exponent: sign * exponent.value },
      unit: powerUnit(value.unit, sign * exponent.value),
      central: value.central ** (sign * exponent.value),
    }
  }

  private primary(): Value {
    const token = this.consume()
    if (token.type === 'number') return { formula: constant(token.value), unit: {}, central: token.value }
    if (token.type === 'name') {
      const variable = this.variables.get(token.value)
      if (!variable) throw new FermiEquationError(`Variable ${token.value} is not defined.`, [token.value])
      this.referencedVariables.add(token.value)
      return variable
    }
    if (token.type === 'operator' && token.value === '(') {
      const value = this.sum()
      if (!this.operator(')')) throw this.error('Expected closing parenthesis')
      this.consume()
      return value
    }
    throw this.error('Expected a variable, number, or parenthesized expression')
  }

  private operator(value: Extract<Token, { type: 'operator' }>['value']) {
    const token = this.peek()
    return token.type === 'operator' && token.value === value
  }

  private peek() {
    return this.tokens[this.index] ?? { type: 'end' as const }
  }

  private consume() {
    const token = this.peek()
    this.index += 1
    return token
  }

  private error(message: string) {
    return new FermiEquationError(`${message} near equation token ${this.index + 1}.`)
  }
}

function constant(value: number): Formula {
  return { type: 'literal', distribution: { type: 'point', value }, unit: {} }
}

function tokenize(source: string) {
  const tokens: Token[] = []
  let index = 0
  while (index < source.length) {
    const character = source[index]!
    if (/\s/.test(character)) {
      index += 1
      continue
    }
    if ('+-*/^()'.includes(character)) {
      tokens.push({ type: 'operator', value: character as Extract<Token, { type: 'operator' }>['value'] })
      index += 1
      continue
    }
    const name = source.slice(index).match(/^[A-Za-z_][A-Za-z0-9_]*/)?.[0]
    if (name) {
      tokens.push({ type: 'name', value: name })
      index += name.length
      continue
    }
    const number = source.slice(index).match(/^(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?/)?.[0]
    if (number) {
      tokens.push({ type: 'number', value: Number(number) })
      index += number.length
      continue
    }
    throw new FermiEquationError(`Unexpected character ${JSON.stringify(character)} in equation.`)
  }
  tokens.push({ type: 'end' })
  return tokens
}