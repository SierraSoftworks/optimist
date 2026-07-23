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

async function submit() {
  document.body.querySelector<HTMLFormElement>('.node-dialog')!
    .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))
  await nextTick()
}

describe('CreateNodeDialog', () => {
  it('creates factors ready for later Squiggle estimate authoring', async () => {
    const wrapper = mount(CreateNodeDialog, {
      props: { open: true, pending: false },
      attachTo: document.body,
    })
    await setInput('input[placeholder="Fast feedback"]', 'Fast feedback')
    await submit()

    const input = wrapper.emitted('submit')?.[0]?.[0]
    expect(input).toMatchObject({
      name: 'fast_feedback',
      payload: {
        kind: 'factor',
        properties: {
          current: null,
        },
      },
    })
    wrapper.unmount()
  })

  it('creates interventions with unset Squiggle-authored planning estimates', async () => {
    const wrapper = mount(CreateNodeDialog, {
      props: { open: true, pending: false },
      attachTo: document.body,
    })
    const intervention = document.body.querySelector<HTMLInputElement>('input[value="intervention"]')!
    intervention.checked = true
    intervention.dispatchEvent(new Event('change', { bubbles: true }))
    await setInput('input[placeholder="Fast feedback"]', 'Improve pipeline')
    await submit()

    const input = wrapper.emitted('submit')?.[0]?.[0] as any
    expect(input.payload.kind).toBe('intervention')
    expect(input.payload.properties.duration).toBeNull()
    expect(input.payload.properties.probability_of_success).toBeNull()
    wrapper.unmount()
  })

  it('creates a bounded metric for later native Squiggle estimation', async () => {
    const wrapper = mount(CreateNodeDialog, {
      props: { open: true, pending: false },
      attachTo: document.body,
    })
    const metric = document.body.querySelector<HTMLInputElement>('input[value="metric"]')!
    metric.checked = true
    metric.dispatchEvent(new Event('change', { bubbles: true }))
    await setInput('input[placeholder="Fast feedback"]', 'Lead time')
    await setInput('input[placeholder="minutes"]', 'days')
    const support = document.body.querySelector<HTMLSelectElement>('.node-setup select')!
    support.value = 'bounded'
    support.dispatchEvent(new Event('change', { bubbles: true }))
    await nextTick()
    const bounds = document.body.querySelectorAll<HTMLInputElement>('.node-setup input[type="number"]')
    bounds[0]!.value = '0'
    bounds[0]!.dispatchEvent(new Event('input', { bubbles: true }))
    bounds[1]!.value = '30'
    bounds[1]!.dispatchEvent(new Event('input', { bubbles: true }))
    await submit()

    expect(wrapper.emitted('submit')?.[0]?.[0]).toMatchObject({
      payload: {
        kind: 'metric',
        properties: {
          unit: 'days',
          dimension: { day: 1 },
          support: { type: 'bounded', lower: 0, upper: 30 },
          current: null,
        },
      },
    })
    wrapper.unmount()
  })
})