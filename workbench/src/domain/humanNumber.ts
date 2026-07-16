export function parseHumanNumber(source: string) {
  const normalized = source.trim().replaceAll(',', '')
  const match = normalized.match(/^([+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?)\s*([kmbt])?$/i)
  if (!match) throw new Error('Use a number such as 138, 1.5M, or 2.4e6.')
  const multiplier = { k: 1e3, m: 1e6, b: 1e9, t: 1e12 }[match[2]?.toLowerCase() as 'k' | 'm' | 'b' | 't'] ?? 1
  const value = Number(match[1]) * multiplier
  if (!Number.isFinite(value)) throw new Error('The estimate must be finite.')
  return value
}

export function formatHumanNumber(value: number) {
  const magnitude = Math.abs(value)
  const suffixes: Array<[number, string]> = [[1e12, 'T'], [1e9, 'B'], [1e6, 'M'], [1e3, 'K']]
  for (const [threshold, suffix] of suffixes) {
    if (magnitude >= threshold) return `${Number((value / threshold).toPrecision(4))}${suffix}`
  }
  return Number(value.toPrecision(6)).toString()
}