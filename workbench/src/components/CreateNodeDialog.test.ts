import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import { nextTick } from 'vue'
import CreateNodeDialog from './CreateNodeDialog.vue'

async function setInput(selector: string, value: string) {
  const element = document.body.querySelector<HTMLInputElement>(selector)!
  element.value = value
  element.dispatchEvent(new Event('input', { bubbles: true }))
  await nextTick()
}

async function continueWizard() {
  document.body.querySelector<HTMLButtonElement>('.node-dialog .primary-button')!.click()
  await nextTick()
}

async function submitWizard() {
  document.body.querySelector<HTMLFormElement>('.node-dialog')!
    .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))
  await nextTick()
}

describe('CreateNodeDialog', () => {
  it('creates factors with an embedded current-state estimate', async () => {
    const wrapper = mount(CreateNodeDialog, {
      props: { open: true, pending: false },
      attachTo: document.body,
    })
    await setInput('input[placeholder="Fast feedback"]', 'Fast feedback')
    await continueWizard()
    expect(document.body.textContent).toContain('Simulation baseline')
    await submitWizard()

    const input = wrapper.emitted('submit')?.[0]?.[0]
    expect(input).toMatchObject({
      name: 'fast_feedback',
      payload: {
        kind: 'factor',
        properties: {
          current: {
            id: 'A',
            revision: 0,
            distribution: { type: 'beta', alpha: 2, beta: 2 },
            source: { type: 'distribution' },
          },
        },
      },
    })
    wrapper.unmount()
  })

  it('creates interventions with planning estimates', async () => {
    const wrapper = mount(CreateNodeDialog, {
      props: { open: true, pending: false },
      attachTo: document.body,
    })
    const intervention = document.body.querySelector<HTMLInputElement>('input[value="intervention"]')!
    intervention.checked = true
    intervention.dispatchEvent(new Event('change', { bubbles: true }))
    await setInput('input[placeholder="Fast feedback"]', 'Improve pipeline')
    await continueWizard()
    expect(document.body.textContent).toContain('Planning estimates')
    await submitWizard()

    const input = wrapper.emitted('submit')?.[0]?.[0] as any
    expect(input.payload.kind).toBe('intervention')
    expect(input.payload.properties.duration.id).toBe('A')
    expect(input.payload.properties.probability_of_success.id).toBe('B')
    wrapper.unmount()
  })
})