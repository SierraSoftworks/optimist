import { describe, expect, it } from 'vitest'
import type { NodeKind } from '../api/types'
import { forceLayout, type LayoutEdge, type LayoutNode } from './graphLayout'

function node(id: string, kind: NodeKind = 'factor'): LayoutNode {
  return { id, kind }
}

function edge(source: string, destination: string): LayoutEdge {
  return { source, destination }
}

describe('forceLayout', () => {
  it('returns nothing for an empty graph', () => {
    expect(forceLayout([], []).size).toBe(0)
  })

  /** The two ends of every question the tool answers must stay where expected. */
  it('anchors interventions on the top row and outcomes on the bottom', () => {
    const nodes = [
      node('i1', 'intervention'),
      node('i2', 'intervention'),
      node('f1'),
      node('m1', 'metric'),
      node('o1', 'outcome'),
    ]
    const positions = forceLayout(nodes, [
      edge('i1', 'f1'), edge('i2', 'f1'), edge('f1', 'm1'), edge('m1', 'o1'),
    ])
    const y = (id: string) => positions.get(id)!.y
    expect(y('i1')).toBe(y('i2'))
    for (const id of ['f1', 'm1', 'o1']) expect(y(id)).toBeGreaterThan(y('i1'))
    for (const id of ['i1', 'i2', 'f1', 'm1']) expect(y('o1')).toBeGreaterThan(y(id))
  })

  it('orders the middle by depth along the causal path', () => {
    const nodes = [node('i', 'intervention'), node('a'), node('b'), node('c'), node('o', 'outcome')]
    const positions = forceLayout(nodes, [
      edge('i', 'a'), edge('a', 'b'), edge('b', 'c'), edge('c', 'o'),
    ])
    expect(positions.get('a')!.y).toBeLessThan(positions.get('b')!.y)
    expect(positions.get('b')!.y).toBeLessThan(positions.get('c')!.y)
  })

  it('spaces interventions evenly across the x axis', () => {
    const nodes = [
      node('i1', 'intervention'), node('i2', 'intervention'), node('i3', 'intervention'),
      node('f1'), node('f2'), node('f3'), node('o', 'outcome'),
    ]
    const positions = forceLayout(nodes, [
      edge('i1', 'f1'), edge('i2', 'f2'), edge('i3', 'f3'),
      edge('f1', 'o'), edge('f2', 'o'), edge('f3', 'o'),
    ])
    const xs = ['i1', 'i2', 'i3']
      .map((id) => positions.get(id)!.x)
      .sort((left, right) => left - right)
    expect(xs[1]! - xs[0]!).toBeCloseTo(xs[2]! - xs[1]!, 6)
  })

  it('centres a lone intervention over the model it acts on', () => {
    const nodes = [node('i', 'intervention'), node('f1'), node('f2'), node('o', 'outcome')]
    const positions = forceLayout(nodes, [edge('i', 'f1'), edge('f1', 'f2'), edge('f2', 'o')])
    const others = ['f1', 'f2', 'o'].map((id) => positions.get(id)!.x)
    const centre = others.reduce((total, x) => total + x, 0) / others.length
    expect(positions.get('i')!.x).toBeCloseTo(centre, 6)
  })

  /**
   * The whole point of the force step: two subgraphs that share no relationship
   * must not interleave, whatever order the nodes arrive in.
   */
  it('clusters connected nodes and keeps unrelated ones apart', () => {
    const nodes = [
      node('i', 'intervention'),
      node('leftA'), node('leftB'), node('rightA'), node('rightB'),
      node('o', 'outcome'),
    ]
    const positions = forceLayout(nodes, [
      edge('i', 'leftA'), edge('leftA', 'leftB'), edge('leftB', 'o'),
      edge('i', 'rightA'), edge('rightA', 'rightB'), edge('rightB', 'o'),
      edge('leftA', 'leftB'), edge('rightA', 'rightB'),
    ])
    const gap = (a: string, b: string) => Math.abs(positions.get(a)!.x - positions.get(b)!.x)
    expect(gap('leftA', 'leftB')).toBeLessThan(gap('leftA', 'rightB'))
    expect(gap('rightA', 'rightB')).toBeLessThan(gap('rightA', 'leftB'))
  })

  it('never overlaps two nodes sharing a row', () => {
    const nodes = [
      node('i', 'intervention'),
      ...Array.from({ length: 8 }, (_, index) => node(`f${index}`)),
      node('o', 'outcome'),
    ]
    const positions = forceLayout(
      nodes,
      nodes.slice(1, -1).flatMap((member) => [edge('i', member.id), edge(member.id, 'o')]),
    )
    const rows = new Map<number, number[]>()
    for (const { x, y } of positions.values()) {
      const row = rows.get(y) ?? []
      row.push(x)
      rows.set(y, row)
    }
    for (const row of rows.values()) {
      row.sort((left, right) => left - right)
      for (let index = 1; index < row.length; index += 1) {
        expect(row[index]! - row[index - 1]!).toBeGreaterThanOrEqual(131)
      }
    }
  })

  it('leaves room between rows for a node and its label', () => {
    const nodes = [node('i', 'intervention'), node('a'), node('b'), node('c'), node('o', 'outcome')]
    const positions = forceLayout(nodes, [
      edge('i', 'a'), edge('a', 'b'), edge('b', 'c'), edge('c', 'o'),
    ])
    const rows = [...new Set([...positions.values()].map((position) => position.y))]
      .sort((left, right) => left - right)
    for (let index = 1; index < rows.length; index += 1) {
      expect(rows[index]! - rows[index - 1]!).toBeGreaterThanOrEqual(107)
    }
  })

  /**
   * Depth says nothing about width. A model whose nodes all sit at one depth —
   * or one with no relationships at all — would otherwise lay out as a single
   * line thousands of pixels wide, which is unreadable at any zoom.
   */
  it('wraps a row that would be wider than the drawing is tall', () => {
    const nodes = Array.from({ length: 100 }, (_, index) => node(`f${index}`))
    const positions = forceLayout(nodes, [])
    const rows = new Set([...positions.values()].map((position) => position.y))
    expect(rows.size).toBeGreaterThan(4)
    const xs = [...positions.values()].map((position) => position.x)
    const width = Math.max(...xs) - Math.min(...xs)
    const ys = [...positions.values()].map((position) => position.y)
    const height = Math.max(...ys) - Math.min(...ys)
    expect(width).toBeLessThan(height * 3)

    // Wrapped rows stack rather than staircasing across the canvas.
    const centres = new Map<number, number[]>()
    for (const { x, y } of positions.values()) centres.set(y, [...(centres.get(y) ?? []), x])
    const rowCentres = [...centres.values()]
      .map((row) => row.reduce((total, x) => total + x, 0) / row.length)
    expect(Math.max(...rowCentres) - Math.min(...rowCentres)).toBeLessThan(width / 4)
  })

  it('keeps interventions on the top row and outcomes on the bottom when rows wrap', () => {
    const nodes = [
      ...Array.from({ length: 12 }, (_, index) => node(`i${index}`, 'intervention')),
      ...Array.from({ length: 30 }, (_, index) => node(`f${index}`)),
      ...Array.from({ length: 8 }, (_, index) => node(`o${index}`, 'outcome')),
    ]
    const edges = Array.from({ length: 30 }, (_, index) => [
      edge(`i${index % 12}`, `f${index}`),
      edge(`f${index}`, `o${index % 8}`),
    ]).flat()
    const positions = forceLayout(nodes, edges)
    const topmost = Math.min(...[...positions.values()].map((position) => position.y))
    const bottom = Math.max(...[...positions.values()].map((position) => position.y))
    for (let index = 0; index < 12; index += 1) {
      expect(positions.get(`i${index}`)!.y).toBeLessThan((topmost + bottom) / 2)
    }
    for (let index = 0; index < 8; index += 1) {
      expect(positions.get(`o${index}`)!.y).toBeGreaterThan((topmost + bottom) / 2)
    }
  })

  /** A re-render must not shuffle the canvas under the reader. */
  it('is deterministic', () => {
    const nodes = [node('i', 'intervention'), node('a'), node('b'), node('o', 'outcome')]
    const edges = [edge('i', 'a'), edge('a', 'b'), edge('b', 'o')]
    expect([...forceLayout(nodes, edges)]).toEqual([...forceLayout(nodes, edges)])
  })

  /** These models have feedback loops, so layering cannot assume acyclicity. */
  it('terminates on a cyclic graph', () => {
    const nodes = [node('i', 'intervention'), node('a'), node('b'), node('o', 'outcome')]
    const positions = forceLayout(nodes, [
      edge('i', 'a'), edge('a', 'b'), edge('b', 'a'), edge('b', 'o'),
    ])
    expect(positions.size).toBe(4)
    for (const position of positions.values()) {
      expect(Number.isFinite(position.x)).toBe(true)
      expect(Number.isFinite(position.y)).toBe(true)
    }
  })

  it('places a node no intervention reaches without losing it', () => {
    const nodes = [node('i', 'intervention'), node('orphan'), node('o', 'outcome')]
    const positions = forceLayout(nodes, [edge('i', 'o')])
    const orphan = positions.get('orphan')!
    expect(Number.isFinite(orphan.x)).toBe(true)
    expect(orphan.y).toBeGreaterThan(positions.get('i')!.y)
    expect(orphan.y).toBeLessThan(positions.get('o')!.y)
  })

  /** A filtered view must lay out as the graph the reader can see. */
  it('ignores relationships to nodes that are not shown', () => {
    const nodes = [node('i', 'intervention'), node('a'), node('o', 'outcome')]
    const visible = forceLayout(nodes, [edge('i', 'a'), edge('a', 'o')])
    const withHidden = forceLayout(nodes, [
      edge('i', 'a'), edge('a', 'o'), edge('a', 'hidden'), edge('hidden', 'o'),
    ])
    expect([...withHidden]).toEqual([...visible])
  })
})
