import type { GraphEdge, GraphNode, StateRelation, Unit } from '../api/types'
import { formatUnitExpression } from './unitExpression'

/**
 * One name a node equation may reference, with the unit it carries.
 *
 * The bindings are derived from the graph rather than authored, so this mirrors
 * what the server will offer the equation. Showing them beside the editor is
 * what makes an equation writable without guessing: a name that is not on this
 * list will be rejected on save.
 */
export interface RelationBinding {
  name: string
  unit: string
  kind: 'baseline' | 'parent' | 'activation'
  /** Owning node title, so a binding can be recognised in a large graph. */
  title: string
}

export function nodeQuantity(node: GraphNode) {
  return node.payload.kind === 'metric' ? node.payload.properties.quantity : node.native_state?.quantity
}

export function nodeRelation(node: GraphNode): StateRelation | null {
  const relation =
    node.payload.kind === 'metric' ? node.payload.properties.relation : node.native_state?.relation
  return relation ?? null
}

/** Reports whether a node kind can own an equation at all. */
export function canOwnRelation(node: GraphNode) {
  if (node.payload.kind === 'metric') return true
  return (
    (node.payload.kind === 'factor' || node.payload.kind === 'outcome') &&
    Boolean(node.native_state)
  )
}

/**
 * Lists every name the server will bind for `node`, in the order they appear.
 *
 * `baseline` comes first because a relative equation starts from it, then
 * parents in graph order, then interventions. Parameters are excluded: the
 * author owns those and adds them alongside the source.
 */
export function relationBindings(
  node: GraphNode,
  nodes: GraphNode[],
  edges: GraphEdge[],
): RelationBinding[] {
  const quantity = nodeQuantity(node)
  const bindings: RelationBinding[] = [{
    name: 'baseline',
    unit: unitText(quantity?.dimension),
    kind: 'baseline',
    title: node.title,
  }]
  for (const edge of edges) {
    if (edge.destination !== node.id) continue
    const source = nodes.find((candidate) => candidate.id === edge.source)
    if (!source) continue
    if (edge.payload.kind === 'contributes' || edge.payload.kind === 'blocks') {
      bindings.push({
        name: source.name,
        unit: unitText(nodeQuantity(source)?.dimension),
        kind: 'parent',
        title: source.title,
      })
    } else if (edge.payload.kind === 'changes') {
      bindings.push({ name: source.name, unit: '1', kind: 'activation', title: source.title })
    }
  }
  return bindings
}

function unitText(unit: Unit | undefined) {
  return unit === undefined ? 'no declared unit' : formatUnitExpression(unit) || '1'
}
