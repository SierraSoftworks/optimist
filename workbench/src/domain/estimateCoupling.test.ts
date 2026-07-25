import { describe, expect, it } from 'vitest'

import type { EstimateAddress, ProjectDependenceModel } from '../api/types'
import {
  CouplingConflict,
  groupOf,
  isSharedQuantity,
  partnersOf,
  sameAddress,
  shareQuantity,
  stopSharing,
} from './estimateCoupling'

function node(id: string, estimate = 'A'): EstimateAddress {
  return { project: 'A', owner: { kind: 'node', id }, estimate }
}

function edge(source: string, destination: string): EstimateAddress {
  return {
    project: 'A',
    owner: { kind: 'edge', id: { source, kind: 'contributes', destination } },
    estimate: 'A',
  }
}

function authored(members: EstimateAddress[]): ProjectDependenceModel {
  return {
    revision: 3,
    residual_groups: [{
      members,
      correlation: { scale: 'latent', matrix: [[1, 0.4], [0.4, 1]] },
    }],
  }
}

describe('sameAddress', () => {
  it('compares node and edge owners without matching across kinds', () => {
    expect(sameAddress(node('B'), node('B'))).toBe(true)
    expect(sameAddress(node('B'), node('C'))).toBe(false)
    expect(sameAddress(node('B'), node('B', 'C'))).toBe(false)
    expect(sameAddress(edge('B', 'C'), edge('B', 'C'))).toBe(true)
    expect(sameAddress(edge('B', 'C'), edge('B', 'D'))).toBe(false)
    expect(sameAddress(node('B'), edge('B', 'C'))).toBe(false)
  })
})

describe('shareQuantity', () => {
  it('creates a unit correlation between two previously uncoupled estimates', () => {
    const model = shareQuantity(null, node('B'), node('C'))
    expect(model.residual_groups).toHaveLength(1)
    expect(model.residual_groups[0]!.correlation).toEqual({
      scale: 'latent',
      matrix: [[1, 1], [1, 1]],
    })
    expect(isSharedQuantity(model.residual_groups[0]!)).toBe(true)
  })

  it('extends an existing shared quantity rather than creating a second group', () => {
    const first = shareQuantity(null, node('B'), node('C'))
    const second = shareQuantity(first, node('D'), node('B'))
    expect(second.residual_groups).toHaveLength(1)
    expect(second.residual_groups[0]!.members).toHaveLength(3)
    expect(second.residual_groups[0]!.correlation.matrix).toEqual([
      [1, 1, 1],
      [1, 1, 1],
      [1, 1, 1],
    ])
    expect(partnersOf(second, node('D'))).toHaveLength(2)
  })

  it('preserves the document revision so the server can check it', () => {
    const model = shareQuantity({ revision: 7, residual_groups: [] }, node('B'), node('C'))
    expect(model.revision).toBe(7)
  })

  it('is idempotent for estimates already sharing a quantity', () => {
    const first = shareQuantity(null, node('B'), node('C'))
    expect(shareQuantity(first, node('B'), node('C'))).toBe(first)
  })

  it('refuses to overwrite an authored correlation', () => {
    const model = authored([node('B'), node('C')])
    expect(() => shareQuantity(model, node('B'), node('D'))).toThrow(CouplingConflict)
    expect(() => shareQuantity(model, node('D'), node('C'))).toThrow(CouplingConflict)
  })

  it('refuses to merge two separate shared quantities', () => {
    const left = shareQuantity(null, node('B'), node('C'))
    const both = shareQuantity(left, node('D'), node('E'))
    expect(() => shareQuantity(both, node('B'), node('D'))).toThrow(CouplingConflict)
  })
})

describe('stopSharing', () => {
  it('drops a group that would fall below two members', () => {
    const model = shareQuantity(null, node('B'), node('C'))
    expect(stopSharing(model, node('B')).residual_groups).toEqual([])
  })

  it('keeps the remaining members coupled and resizes the matrix', () => {
    const model = shareQuantity(shareQuantity(null, node('B'), node('C')), node('D'), node('B'))
    const reduced = stopSharing(model, node('C'))
    expect(reduced.residual_groups[0]!.members).toHaveLength(2)
    expect(reduced.residual_groups[0]!.correlation.matrix).toEqual([[1, 1], [1, 1]])
    expect(groupOf(reduced, node('C'))).toBeNull()
  })

  it('leaves an uncoupled estimate alone', () => {
    const model = shareQuantity(null, node('B'), node('C'))
    expect(stopSharing(model, node('D'))).toBe(model)
  })
})
