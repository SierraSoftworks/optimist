import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import type { GraphEdge, GraphNode } from '../api/types'
import EditStateEstimateDialog from './EditStateEstimateDialog.vue'

const node = {
  id: 'B', revision: 0, name: 'flow', normalized_name: 'flow', title: 'Flow',
  description: '', aliases: [], metadata: {},
  payload: { kind: 'factor', properties: { current: null, desired: null, controllable: false, evidence: [] } },
} as GraphNode
const edge = {
  source: 'A', source_kind: 'metric', destination: 'B', destination_kind: 'factor',
  revision: 2, description: '', metadata: {},
  payload: {
    kind: 'measures',
    properties: {
      polarity: 'lower_is_better',
      calibration: { type: 'linear', state_zero: 20, state_one: 5 },
      observations: [
        { id: 0, revision: 0, value: 16, unit: 'days', observed_at: '2026-07-01T00:00:00Z', source: 'dashboard', measurement_standard_deviation: null, supersedes: null },
        { id: 1, revision: 0, value: 12.5, unit: 'days', observed_at: '2026-07-01T00:00:00Z', source: 'dashboard', measurement_standard_deviation: null, supersedes: 0 },
      ],
    },
  },
} as GraphEdge

describe('EditStateEstimateDialog', () => {
  it('offers the corrected calibrated reading and records adoption provenance', async () => {
    const wrapper = mount(EditStateEstimateDialog, {
      props: { open: true, pending: false, node, projectId: 'A', edges: [edge] },
      global: { stubs: { Teleport: true } },
    })
    expect(wrapper.text()).toContain('12.5 days → 0.500')
    expect(wrapper.text()).not.toContain('16 days →')
    await wrapper.get('.calibrated-evidence .secondary-button').trigger('click')
    expect((wrapper.get('[aria-label="Value on [0, 1]"]').element as HTMLInputElement).value).toBe('0.5')
    expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toContain('Calibrated observation #1')
  })
})