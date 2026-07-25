import type { Estimate, EstimateAddress, GraphEdge, GraphNode, Unit } from '../api/types'
import { formatUnitExpression } from './unitExpression'

/**
 * One addressable estimate offered when choosing a quantity to share.
 *
 * Only estimates the server can address appear here. A temporal profile's
 * durations and its rebound are replaced as one document rather than through
 * individual addresses, so they cannot join a dependence group and are omitted
 * rather than shown as choices that would fail on save.
 */
export interface CatalogueEntry {
  address: EstimateAddress
  /** Owner and slot, for a picker: `Change frequency · Current`. */
  label: string
  /** Canonical unit text, so a chooser can see what it is committing to. */
  unit: string
  /** Authored Squiggle source, which sharing copies to the coupled estimate. */
  source: string
}

export function estimateCatalogue(
  project: string,
  nodes: GraphNode[],
  edges: GraphEdge[],
): CatalogueEntry[] {
  const entries: CatalogueEntry[] = []
  for (const node of nodes) {
    const owner = { kind: 'node' as const, id: node.id }
    const add = (estimate: Estimate | null | undefined, slot: string) => {
      if (estimate) entries.push(entry(project, owner, estimate, `${node.title} · ${slot}`))
    }
    add(node.native_state?.current, 'Current')
    add(node.native_state?.forecast, 'Forecast')
    if (node.payload.kind === 'metric') add(node.payload.properties.current, 'Current')
    if (node.payload.kind === 'intervention') {
      const properties = node.payload.properties
      for (const cost of properties.costs) add(cost.value, `Cost · ${cost.dimension}`)
      add(properties.duration, 'Duration')
      add(properties.probability_of_success, 'Probability of success')
    }
  }
  for (const edge of edges) {
    const owner = { kind: 'edge' as const, id: identity(edge) }
    const route = `${edge.source} ${edge.payload.kind.replaceAll('_', ' ')} ${edge.destination}`
    const add = (estimate: Estimate | null | undefined, slot: string) => {
      if (estimate) entries.push(entry(project, owner, estimate, `${route} · ${slot}`))
    }
    if (edge.payload.kind === 'contributes' || edge.payload.kind === 'changes') {
      add(edge.payload.properties.response, edge.payload.kind === 'changes' ? 'Multiplier' : 'Elasticity')
      add(edge.payload.properties.lag, 'Lag')
    }
    if (edge.payload.kind === 'blocks') add(edge.payload.properties.degree, 'Degree')
  }
  return entries
}

function entry(
  project: string,
  owner: EstimateAddress['owner'],
  estimate: Estimate,
  label: string,
): CatalogueEntry {
  const unit: Unit = estimate.source.definition.target_unit ?? {}
  return {
    address: { project, owner, estimate: estimate.id },
    label,
    unit: formatUnitExpression(unit),
    source: estimate.source.definition.source,
  }
}

function identity(edge: GraphEdge) {
  return { source: edge.source, kind: edge.payload.kind, destination: edge.destination }
}
