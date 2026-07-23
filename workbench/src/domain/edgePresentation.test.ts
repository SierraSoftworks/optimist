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

  it('summarizes a destination response', () => {
    const edge = {
      payload: {
        kind: 'contributes',
        properties: {
          response: {
            source_change: 0.1,
            source_unit: {},
            destination_change: { distribution: { type: 'point', value: -2 } },
            destination_unit: { day: 1 },
          },
          lag: null,
          mechanism: '',
          evidence: [],
        },
      },
    } as unknown as GraphEdge
    expect(edgeMetadataLabel(edge)).toBe('mean response -2.00')
  })

  it('summarizes empirical Squiggle responses from retained draws', () => {
    const edge = {
      payload: {
        kind: 'changes',
        properties: {
          response: {
            source_change: 1,
            source_unit: {},
            destination_change: { distribution: { type: 'empirical', samples: [-3, -2, -1] } },
            destination_unit: { day: 1 },
          },
          lag: null,
          mechanism: '',
          evidence: [],
        },
      },
    } as unknown as GraphEdge
    expect(edgeMetadataLabel(edge)).toBe('mean response -2.00')
  })
})