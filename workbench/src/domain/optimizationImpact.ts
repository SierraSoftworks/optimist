export type OptimizationDirection = 'maximize' | 'minimize'
export type ImpactTone = 'positive' | 'negative' | 'neutral'

export function relativeImprovement(
  improvement: number | null | undefined,
  baseline: number | null | undefined,
): number | null {
  if (improvement === null || improvement === undefined
    || baseline === null || baseline === undefined || baseline === 0) return null
  return improvement / Math.abs(baseline)
}

export function impactTone(
  rawShift: number | null | undefined,
  direction: OptimizationDirection,
): ImpactTone {
  if (rawShift === null || rawShift === undefined || rawShift === 0) return 'neutral'
  const improves = direction === 'maximize' ? rawShift > 0 : rawShift < 0
  return improves ? 'positive' : 'negative'
}