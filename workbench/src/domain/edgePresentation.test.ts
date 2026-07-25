import { describe, expect, it } from 'vitest'
import type { GraphEdge } from '../api/types'
import { edgeMetadataLabel } from './edgePresentation'

describe('edgePresentation', () => {
  it('summarizes bounded blocking degree', () => {
    const edge = {
      payload: {
        kind: 'blocks',
        properties: {
          degree: { distribution: { type: 'beta', alpha: 3, beta: 1 } },
        },
      },
    } as unknown as GraphEdge
    expect(edgeMetadataLabel(edge)).toBe('mean degree 0.75')
  })

  it('summarizes a proportional elasticity', () => {
    const edge = {
      payload: {
        kind: 'contributes',
        properties: {
          response: { distribution: { type: 'point', value: -0.5 } },
          lag: null,
          mechanism: '',
          evidence: [],
        },
      },
    } as unknown as GraphEdge
    expect(edgeMetadataLabel(edge)).toBe('mean elasticity -0.50')
  })

  it('summarizes empirical Squiggle multipliers from retained draws', () => {
    const edge = {
      payload: {
        kind: 'changes',
        properties: {
          response: { distribution: { type: 'empirical', samples: [0.1, 0.2, 0.3] } },
          lag: null,
          mechanism: '',
          evidence: [],
        },
      },
    } as unknown as GraphEdge
    expect(edgeMetadataLabel(edge)).toBe('mean multiplier ×0.20')
  })
})