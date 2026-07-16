import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import { nextTick } from 'vue'
import type { Distribution } from '../api/types'
import DistributionEditor from './DistributionEditor.vue'

describe('DistributionEditor', () => {
  it('updates its visual explanation as Beta parameters change', async () => {
    const wrapper = mount(DistributionEditor, {
      props: {
        modelValue: { type: 'point', value: 0.5 },
        families: ['point', 'beta'],
        support: 'probability',
      },
      attachTo: document.body,
    })
    await wrapper.get('select').setValue('beta')
    const inputs = wrapper.findAll('input[type="number"]')
    await inputs[0]!.setValue('8')
    await inputs[1]!.setValue('2')
    const emitted = wrapper.emitted('update:modelValue')!
    await wrapper.setProps({ modelValue: emitted.at(-1)![0] as Distribution })
    await nextTick()
    expect(wrapper.get('[aria-label="Beta distribution preview"]').text()).toContain('Mean 0.8')
  })

  it('provides keyboard-accessible family and parameter guidance', async () => {
    const wrapper = mount(DistributionEditor, {
      props: {
        modelValue: { type: 'log_normal', location: 0, scale: 0.4 },
        families: ['point', 'log_normal'],
        support: 'non_negative',
      },
    })
    const familyHelp = wrapper.get('button[aria-label="Explain Distribution family"]')
    expect(familyHelp.attributes('aria-expanded')).toBe('false')
    await familyHelp.trigger('click')
    expect(familyHelp.attributes('aria-expanded')).toBe('true')
    expect(document.body.textContent).toContain('overruns can be much larger than underruns')
    expect(wrapper.find('.parameter-popover').exists()).toBe(false)

    await wrapper.get('button[aria-label="Explain Log scale"]').trigger('click')
    expect(document.body.textContent).toContain('tail risk')
    wrapper.unmount()
  })
})
