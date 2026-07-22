import type { EdgeKind, GraphNode } from '../api/types'
import { edgeKinds } from './edgeAuthoring'

export function tokenizeCommand(input: string): { tokens: string[]; incompleteQuote: boolean } {
  const tokens: string[] = []
  let current = ''
  let quote: '"' | "'" | null = null
  let escaped = false
  for (const character of input.trim()) {
    if (escaped) {
      current += character
      escaped = false
    } else if (character === '\\') {
      escaped = true
    } else if (quote) {
      if (character === quote) quote = null
      else current += character
    } else if (character === '"' || character === "'") {
      quote = character
    } else if (/\s/.test(character)) {
      if (current) {
        tokens.push(current)
        current = ''
      }
    } else {
      current += character
    }
  }
  if (current) tokens.push(current)
  return { tokens, incompleteQuote: quote !== null }
}

export function normalizedEdgeKind(value: string): EdgeKind | null {
  const normalized = value.toLocaleLowerCase().replaceAll('-', '_') as EdgeKind
  return edgeKinds.some(({ kind }) => kind === normalized) ? normalized : null
}

export function resolveCommandNode(value: string, nodes: GraphNode[]): GraphNode | null {
  const query = value.toLocaleLowerCase()
  const matches = nodes.filter((node) =>
    node.id.toLocaleLowerCase() === query ||
    node.name.toLocaleLowerCase() === query ||
    node.title.toLocaleLowerCase() === query,
  )
  return matches.length === 1 ? matches[0]! : null
}