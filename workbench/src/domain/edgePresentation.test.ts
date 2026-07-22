import { describe, expect, it } from 'vitest'
import type { GraphEdge } from '../api/types'
import { edgeDisplayLabel, edgeMetadataLabel } from './edgePresentation'

describe('edgePresentation', () => {
  it('shows signed causal means', () => {
    const edge = {
      payload: {
        kind: 'contributes',
        properties: {
          effect: { distribution: { type: 'point', value: 0.45 } },
          lag: null,
          mechanism: '',
          evidence: [],
        },
      },
    } as unknown as GraphEdge
    expect(edgeMetadataLabel(edge)).toBe('mean effect +0.45')
    expect(edgeDisplayLabel(edge)).toBe('contributes · mean effect +0.45')
  })

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

  it('distinguishes a native destination response from normalized effect strength', () => {
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
})