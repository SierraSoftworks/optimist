import type { EditableEdgePayload, EdgeKind, GraphNode, NodeKind } from '../api/types'

export const edgeKinds: Array<{ kind: EdgeKind; label: string }> = [
  { kind: 'contributes', label: 'Contributes' },
  { kind: 'measures', label: 'Measures' },
  { kind: 'changes', label: 'Changes' },
  { kind: 'requires', label: 'Requires' },
  { kind: 'part_of', label: 'Part of' },
  { kind: 'blocks', label: 'Blocks' },
  { kind: 'conflicts_with', label: 'Conflicts with' },
  { kind: 'synergizes_with', label: 'Synergizes with' },
]

export function endpointsAreValid(kind: EdgeKind, source: NodeKind, destination: NodeKind) {
  switch (kind) {
    case 'contributes':
      return (
        ['factor', 'outcome'].includes(source) &&
        ['factor', 'metric', 'outcome'].includes(destination)
      )
    case 'measures':
      return source === 'metric' && ['factor', 'outcome'].includes(destination)
    case 'changes':
      return source === 'intervention' && destination === 'factor'
    case 'requires':
      return (
        ['factor', 'intervention'].includes(source) &&
        ['factor', 'intervention'].includes(destination)
      )
    case 'part_of':
      return source === 'factor' && destination === 'factor'
    case 'blocks':
      return source === 'factor' && ['factor', 'intervention'].includes(destination)
    case 'conflicts_with':
    case 'synergizes_with':
      return source === 'intervention' && destination === 'intervention'
  }
}

export function sourcesFor(kind: EdgeKind, nodes: GraphNode[]) {
  return nodes.filter((source) =>
    nodes.some(
      (destination) =>
        destination.id !== source.id &&
        endpointsAreValid(kind, source.payload.kind, destination.payload.kind),
    ),
  )
}

export function destinationsFor(kind: EdgeKind, source: GraphNode | undefined, nodes: GraphNode[]) {
  if (!source) return []
  return nodes.filter(
    (destination) =>
      destination.id !== source.id &&
      endpointsAreValid(kind, source.payload.kind, destination.payload.kind),
  )
}

interface PayloadInput {
  kind: EdgeKind
  effect: number
  lag: number | null
  mechanism: string
  evidence: string
  polarity: 'higher_is_better' | 'lower_is_better' | 'target_range'
  hard: boolean
  threshold: number | null
}

function estimate(id: string, value: number) {
  return {
    id,
    revision: 0,
    distribution: { type: 'point' as const, value },
    provenance: [],
  }
}

export function edgePayload(input: PayloadInput): EditableEdgePayload {
  switch (input.kind) {
    case 'contributes':
    case 'changes':
      return {
        kind: input.kind,
        properties: {
          effect: estimate('A', input.effect),
          lag: input.lag === null ? null : estimate('B', input.lag),
          mechanism: input.mechanism,
          evidence: input.evidence
            .split('\n')
            .map((value) => value.trim())
            .filter(Boolean),
        },
      }
    case 'measures':
      return { kind: 'measures', properties: { polarity: input.polarity, observations: [] } }
    case 'requires':
      return {
        kind: 'requires',
        properties: { hard: input.hard, satisfaction_threshold: input.threshold },
      }
    case 'blocks':
      return { kind: 'blocks', properties: { degree: estimate('A', input.effect) } }
    case 'part_of':
      return { kind: 'part_of' }
    case 'conflicts_with':
      return { kind: 'conflicts_with' }
    case 'synergizes_with':
      return { kind: 'synergizes_with' }
  }
}
