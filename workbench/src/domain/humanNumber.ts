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

const SI_PREFIXES: Array<[number, string]> = [
  [1e12, 'T'],
  [1e9, 'G'],
  [1e6, 'M'],
  [1e3, 'k'],
  [1, ''],
  [1e-3, 'm'],
  [1e-6, 'µ'],
  [1e-9, 'n'],
  [1e-12, 'p'],
]

/**
 * Renders a value with an SI magnitude prefix, for places with no room to spare.
 *
 * A chart axis has a few characters of gutter, and a quantity plotted over
 * several decades reaches both ends of what plain decimals can write compactly:
 * `0.000137` and `1370000` are each wide enough to run off the frame, while
 * `137µ` and `1.37M` are not. Unlike [`formatHumanNumber`], which uses the
 * financial `B` for billions, this follows SI so that `k`, `M`, and `G` mean what
 * a reader of a measured quantity expects.
 */
export function formatSiNumber(value: number, digits = 3) {
  if (!Number.isFinite(value)) return '—'
  if (value === 0) return '0'
  const magnitude = Math.abs(value)
  if (magnitude < 1e-15) return value.toExponential(1)
  const [factor, prefix] = SI_PREFIXES.find(([threshold]) => magnitude >= threshold)
    ?? SI_PREFIXES[SI_PREFIXES.length - 1]!
  return `${Number((value / factor).toPrecision(digits))}${prefix}`
}