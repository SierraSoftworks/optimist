import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import FermiVariableEditor from './FermiVariableEditor.vue'

describe('FermiVariableEditor', () => {
  it('accepts compact estimates and expands into custom uncertainty controls', async () => {
    const wrapper = mount(FermiVariableEditor, {
      props: {
        modelValue: { id: 0, name: 'people', likely: 1, low: 0.1, high: 10, unit: 'people', mode: 'order_of_magnitude' },
        index: 0,
        removable: false,
      },
    })
    expect(wrapper.text()).toContain('90% interval defaults to 0.1 to 10 people')
    await wrapper.get('[aria-label="Variable 1 estimate"]').setValue('1.5M')
    await wrapper.get('[aria-label="Variable 1 estimate"]').trigger('change')
    expect(wrapper.emitted('update:modelValue')!.at(-1)![0]).toMatchObject({ likely: 1_500_000 })
    await wrapper.get('[aria-label="Edit uncertainty for variable 1"]').trigger('click')
    expect(wrapper.get('[aria-label="Variable 1 uncertainty"]').isVisible()).toBe(true)
  })
})