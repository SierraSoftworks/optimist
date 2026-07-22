import type { GraphNode, NodeKind } from '../api/types'
import type { WorkbenchMode } from '../stores/workbench'
import { destinationsFor, edgeKinds, endpointsAreValid } from './edgeAuthoring'
import { normalizedEdgeKind, resolveCommandNode, tokenizeCommand } from './commandBarSyntax'

export interface CommandSuggestion {
  value: string
  label: string
  detail: string
}

const modes: WorkbenchMode[] = ['explore', 'impediments', 'feedback', 'optimize']
const nodeKinds: NodeKind[] = ['factor', 'outcome', 'metric', 'intervention']

export function commandSuggestions(input: string, nodes: GraphNode[]): CommandSuggestion[] {
  const { tokens } = tokenizeCommand(input)
  const query = input.trim().toLocaleLowerCase()
  if (!tokens.length || tokens.length === 1 && !input.endsWith(' ')) {
    return [
      { value: 'add factor "Fast feedback"', label: 'add factor', detail: 'Create a simulation-ready factor' },
      { value: 'connect ', label: 'connect', detail: 'Create a typed relationship' },
      { value: 'select ', label: 'select', detail: 'Inspect a node' },
      { value: 'mode explore', label: 'mode', detail: 'Switch analysis mode' },
    ].filter((item) => !query || item.label.startsWith(query))
  }
  if (tokens[0] === 'add' && tokens.length <= 2) {
    return nodeKinds.map((kind) => ({
      value: `add ${kind} `,
      label: kind,
      detail: kind === 'metric' ? 'Title and unit required' : 'Probabilistic fields start unset',
    })).filter((item) => !tokens[1] || item.label.startsWith(tokens[1]))
  }
  if (tokens[0] === 'mode') {
    return modes.map((mode) => ({ value: `mode ${mode}`, label: mode, detail: 'Analysis mode' }))
      .filter((item) => !tokens[1] || item.label.startsWith(tokens[1]))
  }
  if (tokens[0] === 'connect' && tokens.length === 2 && input.endsWith(' ')) {
    return relationshipKindSuggestions(tokens[1]!, '', nodes)
  }
  if (tokens[0] === 'connect' && tokens.length === 3 && !input.endsWith(' ')) {
    return relationshipKindSuggestions(tokens[1]!, tokens[2]!, nodes)
  }
  if (tokens[0] === 'connect' && (tokens.length === 3 && input.endsWith(' ') || tokens.length === 4)) {
    const partial = tokens.length === 4 ? tokens[3]!.toLocaleLowerCase() : ''
    const source = resolveCommandNode(tokens[1]!, nodes)
    const kind = normalizedEdgeKind(tokens[2]!)
    return nodes.filter((node) =>
      source && kind && node.id !== source.id &&
      endpointsAreValid(kind, source.payload.kind, node.payload.kind) &&
      (!partial || [node.id, node.name, node.title].some((value) => value.toLocaleLowerCase().includes(partial))),
    ).slice(0, 8).map((node) => ({
      value: `connect ${tokens[1]} ${tokens[2]} ${node.id}`,
      label: `${node.title} · ${node.id}`,
      detail: node.payload.kind,
    }))
  }
  if (tokens[0] === 'select' || tokens[0] === 'connect' && tokens.length <= 2) {
    const prefix = tokens[0] === 'select' ? 'select' : 'connect'
    const partial = tokens[1]?.toLocaleLowerCase() ?? ''
    return nodes.filter((node) =>
      !partial || [node.id, node.name, node.title].some((value) => value.toLocaleLowerCase().includes(partial)),
    ).slice(0, 8).map((node) => ({
      value: `${prefix} ${node.id}${prefix === 'connect' ? ' ' : ''}`,
      label: `${node.title} · ${node.id}`,
      detail: node.payload.kind,
    }))
  }
  return []
}

function relationshipKindSuggestions(
  sourceValue: string,
  partial: string,
  nodes: GraphNode[],
): CommandSuggestion[] {
  const source = resolveCommandNode(sourceValue, nodes)
  if (!source) return []
  const normalizedPartial = partial.toLocaleLowerCase().replaceAll('-', '_')
  return edgeKinds
    .filter(({ kind, label }) =>
      destinationsFor(kind, source, nodes).length > 0 &&
      (!normalizedPartial ||
        kind.startsWith(normalizedPartial) ||
        label.toLocaleLowerCase().replaceAll(' ', '_').startsWith(normalizedPartial)),
    )
    .map(({ kind, label }) => ({
      value: `connect ${sourceValue} ${kind} `,
      label,
      detail: `From ${source.payload.kind}`,
    }))
}