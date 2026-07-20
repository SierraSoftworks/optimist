import type { Distribution, GraphEdge } from '../api/types'

export function distributionMean(distribution: Distribution) {
  if (distribution.type === 'point') return distribution.value ?? 0
  if (distribution.type === 'normal') return distribution.mean ?? 0
  if (distribution.type === 'log_normal') {
    const location = distribution.location ?? 0
    const scale = distribution.scale ?? 0
    return Math.exp(location + scale * scale / 2)
  }
  const alpha = distribution.alpha ?? 1
  const beta = distribution.beta ?? 1
  const proportion = alpha / (alpha + beta)
  if (distribution.type === 'scaled_beta') {
    const lower = distribution.lower ?? 0
    return lower + proportion * ((distribution.upper ?? 1) - lower)
  }
  return proportion
}

export function edgeMetadataLabel(edge: GraphEdge) {
  if (edge.payload.kind === 'contributes' || edge.payload.kind === 'changes') {
    const mean = distributionMean(edge.payload.properties.effect.distribution)
    return `mean effect ${mean >= 0 ? '+' : ''}${mean.toFixed(2)}`
  }
  if (edge.payload.kind === 'blocks') {
    return `mean degree ${distributionMean(edge.payload.properties.degree.distribution).toFixed(2)}`
  }
  if (edge.payload.kind === 'measures') {
    return edge.payload.properties.polarity.replaceAll('_', ' ')
  }
  return edge.description ? 'documented' : 'structural'
}

export function edgeDisplayLabel(edge: GraphEdge) {
  return `${edge.payload.kind.replaceAll('_', ' ')} · ${edgeMetadataLabel(edge)}`
}