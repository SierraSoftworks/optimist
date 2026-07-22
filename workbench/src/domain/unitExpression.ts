import type { Unit } from '../api/types'

type Token =
  | { type: 'name'; value: string }
  | { type: 'integer'; value: number }
  | { type: 'operator'; value: '*' | '/' | '^' | '(' | ')' | '-' }
  | { type: 'end' }

export function parseUnitExpression(source: string): Unit {
  const parser = new UnitParser(tokenize(source.trim()))
  return parser.parse()
}

export function formatUnitExpression(unit: Unit) {
  const numerator: string[] = []
  const denominator: string[] = []
  for (const [name, exponent] of Object.entries(unit).sort(([left], [right]) => left.localeCompare(right))) {
    const target = exponent > 0 ? numerator : denominator
    const magnitude = Math.abs(exponent)
    target.push(magnitude === 1 ? name : `${name}^${magnitude}`)
  }
  const top = numerator.join('*') || '1'
  return denominator.length ? `${top}/${denominator.join('*')}` : top
}

export function formatSquiggleUnitAnnotation(unit: Unit) {
  const compatible: Unit = {}
  for (const [name, exponent] of Object.entries(unit)) {
    compatible[squiggleUnitName(name)] = exponent
  }
  return formatUnitExpression(compatible)
}

function squiggleUnitName(name: string) {
  if (/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) return name
  return `optimist_unit_${Array.from(name, (character) => character.codePointAt(0)!.toString(16)).join('_')}`
}

export function multiplyUnits(left: Unit, right: Unit) {
  return combineUnits(left, right, 1)
}

export function divideUnits(left: Unit, right: Unit) {
  return combineUnits(left, right, -1)
}

export function powerUnit(unit: Unit, exponent: number) {
  const result: Unit = {}
  for (const [name, value] of Object.entries(unit)) {
    const powered = value * exponent
    if (powered !== 0) result[name] = powered
  }
  return result
}

export function unitsEqual(left: Unit, right: Unit) {
  const names = new Set([...Object.keys(left), ...Object.keys(right)])
  return Array.from(names).every((name) => (left[name] ?? 0) === (right[name] ?? 0))
}

function combineUnits(left: Unit, right: Unit, direction: 1 | -1) {
  const result: Unit = { ...left }
  for (const [name, exponent] of Object.entries(right)) {
    const combined = (result[name] ?? 0) + direction * exponent
    if (combined === 0) delete result[name]
    else result[name] = combined
  }
  return result
}

class UnitParser {
  private index = 0
  private readonly tokens: Token[]

  constructor(tokens: Token[]) {
    this.tokens = tokens
  }

  parse() {
    if (this.peek().type === 'end') return {}
    const unit = this.product()
    if (this.peek().type !== 'end') throw this.error('Unexpected token')
    return unit
  }

  private product(): Unit {
    let unit = this.factor()
    while (this.operator('*') || this.operator('/')) {
      const operation = this.consume() as Extract<Token, { type: 'operator' }>
      const right = this.factor()
      unit = operation.value === '*' ? multiplyUnits(unit, right) : divideUnits(unit, right)
    }
    return unit
  }

  private factor(): Unit {
    let unit: Unit
    const token = this.consume()
    if (token.type === 'name') {
      unit = { [canonicalUnitName(token.value)]: 1 }
    } else if (token.type === 'integer' && token.value === 1) {
      unit = {}
    } else if (token.type === 'operator' && token.value === '(') {
      unit = this.product()
      this.expectOperator(')')
    } else {
      throw this.error('Expected a unit name, 1, or parenthesized unit')
    }
    if (!this.operator('^')) return unit
    this.consume()
    let sign = 1
    if (this.operator('-')) {
      this.consume()
      sign = -1
    }
    const exponent = this.consume()
    if (exponent.type !== 'integer') throw this.error('Unit powers must be integers')
    return powerUnit(unit, sign * exponent.value)
  }

  private expectOperator(value: Extract<Token, { type: 'operator' }>['value']) {
    if (!this.operator(value)) throw this.error(`Expected ${value}`)
    this.consume()
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
    return new Error(`${message} in unit expression.`)
  }
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
    if ('*/^()-'.includes(character)) {
      tokens.push({ type: 'operator', value: character as Extract<Token, { type: 'operator' }>['value'] })
      index += 1
      continue
    }
    const name = source.slice(index).match(/^[A-Za-z][A-Za-z0-9_.-]*/)?.[0]
    if (name) {
      tokens.push({ type: 'name', value: name })
      index += name.length
      continue
    }
    const integer = source.slice(index).match(/^\d+/)?.[0]
    if (integer) {
      tokens.push({ type: 'integer', value: Number(integer) })
      index += integer.length
      continue
    }
    throw new Error(`Unexpected character ${JSON.stringify(character)} in unit expression.`)
  }
  tokens.push({ type: 'end' })
  return tokens
}

function canonicalUnitName(value: string) {
  const lower = value.toLowerCase()
  const irregular: Record<string, string> = {
    people: 'person',
    persons: 'person',
  }
  if (irregular[lower]) return irregular[lower]
  if (!/[_.-]/.test(lower) && lower.length > 3 && lower.endsWith('s') && !lower.endsWith('ss')) {
    return lower.slice(0, -1)
  }
  return lower
}