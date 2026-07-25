import type { NodeKind } from '../api/types'
import type { GraphPosition } from './graphView'

/**
 * Force-directed layout for causal graphs.
 *
 * A plain force simulation puts connected nodes near each other but says nothing
 * about direction, so a causal model comes out as a ball in which nothing
 * indicates that interventions act on outcomes. A plain hierarchy respects the
 * direction but places siblings in arbitrary order, so relationships cross the
 * whole canvas and unrelated subgraphs interleave.
 *
 * This does both: vertical position is derived from a node's place in the causal
 * flow, and horizontal position is settled by attraction along relationships and
 * repulsion between neighbours. Interventions sit on the top row and outcomes on
 * the bottom, which are the two ends of every question the tool answers.
 */

/** Minimum horizontal gap between two nodes sharing a row. */
const MIN_GAP = 132

/** Minimum vertical gap between rows, leaving room for a node and its label. */
const MIN_ROW_GAP = 108

/** Vertical extent of the whole layout, before fitting to the viewport. */
const HEIGHT = 640

/** Rows closer together than this repel each other horizontally. */
const REPULSION_BAND = 110

/** Iterations are bounded so a large model cannot stall the canvas. */
const MAX_ITERATIONS = 320

/** Nodes beyond this count skip the simulation and keep their seeded spread. */
const MAX_SIMULATED = 400

const ATTRACTION = 0.08
const REPULSION = 5200
const GRAVITY = 0.015

/** Keeps middle nodes clear of the pinned intervention and outcome rows. */
const MIDDLE_BAND = { top: 0.16, bottom: 0.84 }

export interface LayoutNode {
  id: string
  kind: NodeKind
}

export interface LayoutEdge {
  source: string
  destination: string
}

/** Deterministic PRNG, so the same model always lays out the same way. */
function random(seed: number): () => number {
  let state = seed
  return () => {
    state |= 0
    state = (state + 0x6d2b79f5) | 0
    let value = Math.imul(state ^ (state >>> 15), 1 | state)
    value = (value + Math.imul(value ^ (value >>> 7), 61 | value)) ^ value
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296
  }
}

function adjacency(edges: LayoutEdge[], reverse = false): Map<string, string[]> {
  const result = new Map<string, string[]>()
  for (const edge of edges) {
    const from = reverse ? edge.destination : edge.source
    const to = reverse ? edge.source : edge.destination
    const existing = result.get(from)
    if (existing) existing.push(to)
    else result.set(from, [to])
  }
  return result
}

/** Breadth-first hop counts, which terminate even when the graph has cycles. */
function distances(starts: string[], links: Map<string, string[]>): Map<string, number> {
  const depths = new Map<string, number>()
  const queue: string[] = []
  for (const start of starts) {
    if (depths.has(start)) continue
    depths.set(start, 0)
    queue.push(start)
  }
  for (let head = 0; head < queue.length; head += 1) {
    const id = queue[head]!
    const depth = depths.get(id)!
    for (const next of links.get(id) ?? []) {
      if (depths.has(next)) continue
      depths.set(next, depth + 1)
      queue.push(next)
    }
  }
  return depths
}

/**
 * Places a node between the intervention and outcome rows.
 *
 * A node's height is the share of the causal path it sits at: two hops from an
 * intervention and one from an outcome puts it two thirds of the way down. That
 * keeps the flow readable without needing an acyclic graph, which these models
 * are not. Nodes no intervention reaches and no outcome depends on have no
 * position in the flow and sit in the middle.
 */
function verticalFraction(
  node: LayoutNode,
  fromInterventions: Map<string, number>,
  toOutcomes: Map<string, number>,
): number {
  if (node.kind === 'intervention') return 0
  if (node.kind === 'outcome') return 1
  const down = fromInterventions.get(node.id)
  const up = toOutcomes.get(node.id)
  const fraction = down !== undefined && up !== undefined
    ? (down + up === 0 ? 0.5 : down / (down + up))
    : down !== undefined
      ? down / (down + 1)
      : up !== undefined
        ? 1 - up / (up + 1)
        : 0.5
  return Math.min(MIDDLE_BAND.bottom, Math.max(MIDDLE_BAND.top, fraction))
}

interface Placed {
  id: string
  kind: NodeKind
  x: number
  y: number
  fraction: number
  row: number
}

/**
 * How many nodes a row may hold before it wraps.
 *
 * Depth alone says nothing about width, so a model whose factors all sit one hop
 * from an intervention would lay out as a single line thousands of pixels wide.
 * Balancing the two gaps keeps the drawing roughly as wide as it is tall: `r`
 * rows of `c` nodes measure `c·MIN_GAP` by `r·MIN_ROW_GAP`, so `c ≈ √(n·gap
 * ratio)`. The floor leaves small models on one row, where wrapping would
 * separate siblings for no gain.
 */
function rowCapacity(total: number): number {
  return Math.max(6, Math.ceil(Math.sqrt(total * (MIN_ROW_GAP / MIN_GAP))))
}

/**
 * Turns an ordered list of row depths into y positions far enough apart to read.
 *
 * Depth alone bunches rows wherever the graph is dense: two depths a twelfth
 * apart put a node's label on top of the row beneath it. Rows keep their order,
 * and where their spacing already clears the minimum it is preserved, so a node
 * three quarters of the way down the causal path still looks it.
 */
function spaceRows(fractions: number[]): number[] {
  if (!fractions.length) return []
  const positions: number[] = []
  let previous = Number.NEGATIVE_INFINITY
  for (const fraction of fractions) {
    const y = previous === Number.NEGATIVE_INFINITY
      ? fraction * HEIGHT
      : Math.max(fraction * HEIGHT, previous + MIN_ROW_GAP)
    positions.push(y)
    previous = y
  }
  // Spreading rows apart can push the last one past the intended extent, so the
  // whole column is rescaled to keep outcomes on the bottom row.
  const first = positions[0]!
  const span = positions.at(-1)! - first
  if (span <= 0) return positions.map(() => 0)
  const height = Math.max(HEIGHT, span)
  return positions.map((y) => (y - first) / span * height)
}

/**
 * Splits rows that are too wide and gives every row its vertical position.
 *
 * Members are wrapped in the order the simulation settled them, so a node stays
 * beside the neighbours it is related to rather than being dealt out arbitrarily.
 */
function assignRows(placed: Placed[], capacity: number): void {
  const groups = new Map<number, Placed[]>()
  for (const node of placed) {
    const group = groups.get(node.fraction)
    if (group) group.push(node)
    else groups.set(node.fraction, [node])
  }
  const depths: number[] = []
  for (const fraction of [...groups.keys()].sort((left, right) => left - right)) {
    const members = groups.get(fraction)!
    members.sort((left, right) => left.x - right.x)
    const rows = Math.ceil(members.length / capacity)
    const perRow = Math.ceil(members.length / rows)
    for (const [index, member] of members.entries()) {
      member.row = depths.length + Math.floor(index / perRow)
    }
    if (rows > 1) {
      // Each sub-row holds one slice of the group's width, so left to itself the
      // first would sit far left and the last far right, staircasing down the
      // canvas. Stacking them on the group's centre keeps the block square.
      const centre = members.reduce((total, member) => total + member.x, 0) / members.length
      for (let row = 0; row < rows; row += 1) {
        const slice = members.slice(row * perRow, (row + 1) * perRow)
        if (!slice.length) continue
        const sliceCentre = slice.reduce((total, member) => total + member.x, 0) / slice.length
        for (const member of slice) member.x += centre - sliceCentre
      }
    }
    for (let row = 0; row < rows; row += 1) depths.push(fraction)
  }
  const positions = spaceRows(depths)
  for (const node of placed) node.y = positions[node.row]!
}

/**
 * Pushes apart nodes that share a row, preserving their order and centre.
 *
 * Attraction alone will happily stack two nodes on the same point. Spreading
 * them from the left would drag every row rightwards, so each row keeps the
 * centre of mass the simulation gave it.
 */
function separateRows(placed: Placed[]): void {
  const rows = new Map<number, Placed[]>()
  for (const node of placed) {
    const row = rows.get(node.row)
    if (row) row.push(node)
    else rows.set(node.row, [node])
  }
  for (const row of rows.values()) {
    if (row.length < 2) continue
    row.sort((left, right) => left.x - right.x)
    const before = row.reduce((total, node) => total + node.x, 0) / row.length
    for (let index = 1; index < row.length; index += 1) {
      const previous = row[index - 1]!
      const current = row[index]!
      if (current.x - previous.x < MIN_GAP) current.x = previous.x + MIN_GAP
    }
    const after = row.reduce((total, node) => total + node.x, 0) / row.length
    for (const node of row) node.x += before - after
  }
}

/**
 * Spreads interventions evenly across the width the rest of the model occupies.
 *
 * Their order comes from where the simulation settled them, so an intervention
 * stays above the part of the model it acts on and its relationships do not
 * cross the canvas, while the even spacing keeps the top row readable rather
 * than clumped wherever the forces happened to balance.
 */
function spaceInterventions(placed: Placed[]): void {
  const others = placed.filter((node) => node.kind !== 'intervention')
  const extent = others.length
    ? Math.max(...others.map((node) => Math.abs(node.x))) * 2
    : 0
  const rows = new Map<number, Placed[]>()
  for (const node of placed) {
    if (node.kind !== 'intervention') continue
    const row = rows.get(node.row)
    if (row) row.push(node)
    else rows.set(node.row, [node])
  }
  for (const interventions of rows.values()) {
    interventions.sort((left, right) => left.x - right.x)
    if (interventions.length === 1) {
      interventions[0]!.x = others.length
        ? others.reduce((total, node) => total + node.x, 0) / others.length
        : 0
      continue
    }
    const gap = Math.max(MIN_GAP, extent / (interventions.length - 1))
    const span = gap * (interventions.length - 1)
    for (const [index, node] of interventions.entries()) {
      node.x = index * gap - span / 2
    }
  }
}

/**
 * Lays out a causal graph, clustering by relationship within a causal flow.
 *
 * Only relationships between nodes present in `nodes` are used, so a filtered
 * view lays out as the graph the reader can actually see rather than being
 * pulled about by nodes that are hidden.
 */
export function forceLayout(
  nodes: LayoutNode[],
  edges: LayoutEdge[],
  seed = 0x5eed,
): Map<string, GraphPosition> {
  const positions = new Map<string, GraphPosition>()
  if (!nodes.length) return positions

  const present = new Set(nodes.map((node) => node.id))
  const links = edges.filter((edge) => present.has(edge.source) && present.has(edge.destination))
  const fromInterventions = distances(
    nodes.filter((node) => node.kind === 'intervention').map((node) => node.id),
    adjacency(links),
  )
  const toOutcomes = distances(
    nodes.filter((node) => node.kind === 'outcome').map((node) => node.id),
    adjacency(links, true),
  )

  const next = random(seed)
  const placed: Placed[] = nodes.map((node) => {
    const fraction = verticalFraction(node, fromInterventions, toOutcomes)
    return {
      id: node.id,
      kind: node.kind,
      x: (next() - 0.5) * MIN_GAP * Math.max(2, Math.sqrt(nodes.length)),
      y: fraction * HEIGHT,
      fraction,
      row: 0,
    }
  })
  const index = new Map(placed.map((node) => [node.id, node]))

  if (placed.length <= MAX_SIMULATED) {
    const iterations = Math.max(60, Math.min(MAX_ITERATIONS, Math.round(4000 / placed.length)))
    for (let step = 0; step < iterations; step += 1) {
      const alpha = 1 - step / iterations
      for (const edge of links) {
        const source = index.get(edge.source)!
        const destination = index.get(edge.destination)!
        const pull = (destination.x - source.x) * ATTRACTION * alpha
        source.x += pull
        destination.x -= pull
      }
      for (let left = 0; left < placed.length; left += 1) {
        const first = placed[left]!
        for (let right = left + 1; right < placed.length; right += 1) {
          const second = placed[right]!
          // Nodes on distant rows cannot collide, so they need no repulsion and
          // skipping them keeps this affordable on a large model.
          if (Math.abs(first.y - second.y) > REPULSION_BAND) continue
          const delta = second.x - first.x
          const distance = Math.abs(delta)
          if (distance > MIN_GAP * 3) continue
          const direction = distance < 1e-6 ? (left % 2 ? 1 : -1) : delta / distance
          const force = (REPULSION / Math.max(distance, 24) ** 2) * alpha
          first.x -= direction * force
          second.x += direction * force
        }
      }
      for (const node of placed) node.x -= node.x * GRAVITY * alpha
    }
  }

  assignRows(placed, rowCapacity(placed.length))
  separateRows(placed)
  spaceInterventions(placed)
  separateRows(placed)

  for (const node of placed) positions.set(node.id, { x: node.x, y: node.y })
  return positions
}
