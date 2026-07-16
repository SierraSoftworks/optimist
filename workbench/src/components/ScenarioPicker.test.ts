import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import type { Scenario } from '../api/types'
import ScenarioPicker from './ScenarioPicker.vue'

const config = {
  seed: 42, minimum_samples: 100, maximum_samples: 1000,
  absolute_tolerance: 0.01, relative_tolerance: 0.01,
}
const scenarios: Scenario[] = [
  {
    id: 'A', revision: 1, name: 'delivery', title: 'Delivery', rationale: '',
    objectives: [], planning_horizon: 12, budgets: [], candidate_interventions: [], monte_carlo: config,
  },
  {
    id: 'B', revision: 0, name: 'reliability', title: 'Reliability', rationale: '',
    objectives: [], planning_horizon: 6, budgets: [], candidate_interventions: [], monte_carlo: config,
  },
]

describe('ScenarioPicker', () => {
  it('shows selected scenario metadata and emits a menu selection', async () => {
    const wrapper = mount(ScenarioPicker, {
      props: { scenarios, selectedScenarioId: 'A' },
      attachTo: document.body,
    })
    expect(wrapper.get('.scenario-picker-trigger').text()).toContain('A · r1 · 12 periods')
    await wrapper.get('.scenario-picker-trigger').trigger('click')
    const options = Array.from(document.body.querySelectorAll<HTMLButtonElement>('.scenario-menu [role="option"]'))
    expect(options).toHaveLength(2)
    options[1]!.click()
    expect(wrapper.emitted('select')![0]).toEqual(['B'])
    wrapper.unmount()
  })

  it('supports keyboard movement and a new-scenario command', async () => {
    const wrapper = mount(ScenarioPicker, {
      props: { scenarios, selectedScenarioId: 'A' },
      attachTo: document.body,
    })
    await wrapper.get('.scenario-picker-trigger').trigger('click')
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'End' }))
    const create = document.body.querySelector<HTMLButtonElement>('.scenario-menu-create')!
    create.click()
    expect(wrapper.emitted('create')).toHaveLength(1)
    wrapper.unmount()
  })
})
