import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import type { GraphNode } from '../api/types'
import CreateScenarioDialog from './CreateScenarioDialog.vue'

const nodes: GraphNode[] = [
  {
    id: 'A', revision: 0, name: 'reliability', normalized_name: 'reliability', title: 'Reliability',
    description: '', aliases: [], metadata: {},
    payload: { kind: 'outcome', properties: { direction: 'maximize', current: null, desired: null, evidence: [] } },
  },
  {
    id: 'B', revision: 0, name: 'automate', normalized_name: 'automate', title: 'Automate',
    description: '', aliases: [], metadata: {},
    payload: { kind: 'intervention', properties: { costs: [], duration: null, probability_of_success: null, acceptance_criteria: [] } },
  },
]

describe('CreateScenarioDialog', () => {
  it('emits a typed deterministic scenario draft', async () => {
    const wrapper = mount(CreateScenarioDialog, {
      props: { open: false, pending: false, nodes, scenario: null },
      global: { stubs: { Teleport: true } },
    })
    await wrapper.setProps({ open: true })
    await wrapper.get('input[placeholder="Reliable delivery"]').setValue('Reliable delivery')
    const checkboxes = wrapper.findAll('input[type="checkbox"]')
    await checkboxes[0]!.setValue(true)
    await checkboxes[1]!.setValue(true)
    const invalid = Array.from(wrapper.get('form').element.querySelectorAll(':invalid')).map((element) => ({
      label: element.closest('label')?.textContent?.trim(),
      message: (element as HTMLInputElement).validationMessage,
      value: (element as HTMLInputElement).value,
    }))
    expect(invalid).toEqual([])
    await wrapper.get('form').trigger('submit')
    expect(wrapper.emitted('submit')![0]![0]).toMatchObject({
      name: 'reliable_delivery',
      title: 'Reliable delivery',
      objectives: [{ outcome_id: 'A', direction: 'maximize', importance: 1 }],
      planning_horizon: 12,
      budgets: [],
      candidate_interventions: ['B'],
      monte_carlo: {
        seed: 42,
        minimum_samples: 100,
        maximum_samples: 1000,
        absolute_tolerance: 0.01,
        relative_tolerance: 0.01,
      },
    })
  })

  it('prefills an existing scenario for complete replacement editing', async () => {
    const scenario = {
      id: 'A', revision: 2, name: 'delivery', title: 'Delivery', rationale: 'Current plan',
      objectives: [{ outcome_id: 'A', direction: 'maximize' as const, importance: 2 }],
      planning_horizon: 8, budgets: [], candidate_interventions: ['B'],
      monte_carlo: {
        seed: 7, minimum_samples: 20, maximum_samples: 200,
        absolute_tolerance: 0.02, relative_tolerance: 0.03,
      },
    }
    const wrapper = mount(CreateScenarioDialog, {
      props: { open: false, pending: false, nodes, scenario },
      global: { stubs: { Teleport: true } },
    })
    await wrapper.setProps({ open: true })
    expect(wrapper.get('h2').text()).toBe('Edit scenario')
    expect((wrapper.get('input[placeholder="Reliable delivery"]').element as HTMLInputElement).value).toBe('Delivery')
    expect((wrapper.get('input[aria-label="Importance"]').element as HTMLInputElement).value).toBe('2')
    expect(wrapper.findAll('input[type="checkbox"]').every((input) => (input.element as HTMLInputElement).checked)).toBe(true)
    await wrapper.get('input[placeholder="Reliable delivery"]').setValue('Updated delivery')
    await wrapper.get('form').trigger('submit')
    expect(wrapper.emitted('submit')![0]![0]).toMatchObject({
      name: 'delivery', title: 'Updated delivery', planning_horizon: 8,
      objectives: [{ outcome_id: 'A', importance: 2 }], candidate_interventions: ['B'],
      monte_carlo: { seed: 7, minimum_samples: 20, maximum_samples: 200 },
    })
  })
})
