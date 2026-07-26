import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import type { StructuralAnalysis } from '../api/types'
import FeedbackAnalysisPanel from './FeedbackAnalysisPanel.vue'

function analysis(overrides: Partial<StructuralAnalysis> = {}): StructuralAnalysis {
  return {
    revision: {
      project: 'A', graph_revision: 4, scenario: null,
      dependence_revision: null,
    },
    components: [],
    cycles: [],
    cycles_truncated: false,
    limits: { maximum_cycle_length: 8, maximum_cycles: 1000 },
    ...overrides,
  }
}

describe('FeedbackAnalysisPanel', () => {
  it('explains an acyclic result without implying statistical stability', () => {
    const wrapper = mount(FeedbackAnalysisPanel, {
      props: { analysis: analysis(), loops: [], nodes: [], pending: false, error: null, selectedCycle: null },
    })
    expect(wrapper.text()).toContain('No causal feedback loops')
    expect(wrapper.text()).toContain('acyclic graph')
    expect(wrapper.text()).toContain('g4')
  })

  it('warns when bounded cycle enumeration is truncated', () => {
    const wrapper = mount(FeedbackAnalysisPanel, {
      props: {
        analysis: analysis({
          cycles_truncated: true,
          limits: { maximum_cycle_length: 8, maximum_cycles: 10 },
        }),
        loops: [],
        nodes: [],
        pending: false,
        error: null,
        selectedCycle: null,
      },
    })
    expect(wrapper.text()).toContain('10-cycle limit')
    expect(wrapper.text()).toContain('partial result')
  })

  /**
   * Topology says a loop exists; the gain says whether it matters, and the per
   * hop shares say which relationship is carrying it.
   */
  it('weighs each cycle and names the relationship carrying the compounding', () => {
    const cycle = {
      nodes: ['A', 'B'],
      edges: [
        { source: 'A', source_kind: 'factor', destination: 'B', destination_kind: 'factor', kind: 'contributes' },
        { source: 'B', source_kind: 'factor', destination: 'A', destination_kind: 'factor', kind: 'contributes' },
      ],
    }
    const wrapper = mount(FeedbackAnalysisPanel, {
      props: {
        analysis: analysis({ cycles: [cycle] as never }),
        loops: [{
          states: ['A', 'B'],
          gain: 1.65,
          instability: 0.62,
          weights: [
            { source: 'A', destination: 'B', response: 3.3, contribution: Math.log(3.3) },
            { source: 'B', destination: 'A', response: 0.5, contribution: Math.log(0.5) },
          ],
        }],
        nodes: [
          { id: 'A', title: 'Retries' },
          { id: 'B', title: 'Load' },
        ] as never,
        pending: false,
        error: null,
        selectedCycle: null,
      },
    })

    const text = wrapper.text()
    expect(text).toContain('gain 1.65')
    expect(text).toContain('A deviation grows each trip')
    expect(text).toContain('Retries → Load')
    expect(text).toContain('3.300')
    expect(text).toContain('Runs away in 62% of sampled draws')
    expect(wrapper.get('.loop-gain').attributes('data-tone')).toBe('amplifying')
    // The amplifying hop is marked apart from the damping one.
    const rows = wrapper.findAll('.weight-row')
    expect(rows[0]!.attributes('data-amplifies')).toBe('true')
    expect(rows[1]!.attributes('data-amplifies')).toBe('false')
  })
})
