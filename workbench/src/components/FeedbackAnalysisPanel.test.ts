import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import type { StructuralAnalysis } from '../api/types'
import FeedbackAnalysisPanel from './FeedbackAnalysisPanel.vue'

function analysis(overrides: Partial<StructuralAnalysis> = {}): StructuralAnalysis {
  return {
    revision: {
      project: 'A', graph_revision: 4, scenario: null,
      dependence_revision: null, formula_revision: 0,
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
      props: { analysis: analysis(), pending: false, error: null, selectedCycle: null },
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
        pending: false,
        error: null,
        selectedCycle: null,
      },
    })
    expect(wrapper.text()).toContain('10-cycle limit')
    expect(wrapper.text()).toContain('partial result')
  })
})
