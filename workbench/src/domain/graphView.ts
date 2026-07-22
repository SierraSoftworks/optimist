import type { GraphNode, NodeKind } from '../api/types'

export type GraphDetail = 'overview' | 'context' | 'detail'
export type GraphLayoutMode = 'hierarchy' | 'clusters'
export interface GraphPosition { x: number; y: number }

const kindOrder: NodeKind[] = ['intervention', 'factor', 'metric', 'outcome']

export function graphDetailForZoom(zoom: number): GraphDetail {
  if (zoom < 0.62) return 'overview'
  if (zoom < 0.94) return 'context'
  return 'detail'
}

export function defaultGraphLayout(nodeCount: number): GraphLayoutMode {
  return nodeCount >= 60 ? 'clusters' : 'hierarchy'
}

export function clusteredPositions(nodes: GraphNode[]): Map<string, GraphPosition> {
  const positions = new Map<string, GraphPosition>()
  let bandTop = 0
  for (const kind of kindOrder) {
    const members = nodes.filter((node) => node.payload.kind === kind)
    if (!members.length) continue
    const columns = Math.max(1, Math.ceil(Math.sqrt(members.length * 1.8)))
    const rows = Math.ceil(members.length / columns)
    const width = (columns - 1) * 112
    for (const [index, node] of members.entries()) {
      positions.set(node.id, {
        x: (index % columns) * 112 - width / 2,
        y: bandTop + Math.floor(index / columns) * 92,
      })
    }
    bandTop += Math.max(180, rows * 92 + 72)
  }
  return positions
}