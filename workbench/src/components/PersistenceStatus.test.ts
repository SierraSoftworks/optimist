import { mount } from '@vue/test-utils'
import { ref } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useServerHealth } from '../composables/useProjectData'
import PersistenceStatus from './PersistenceStatus.vue'

vi.mock('../composables/useProjectData', () => ({
  useServerHealth: vi.fn(),
}))

describe('PersistenceStatus', () => {
  beforeEach(() => {
    vi.mocked(useServerHealth).mockReturnValue({
      data: ref({ status: 'ok', version: '0.1.0', persistence: { state: 'idle' } }),
    } as ReturnType<typeof useServerHealth>)
  })

  it('keeps an idle health poll out of the app header', () => {
    const wrapper = mount(PersistenceStatus)
    expect(wrapper.text()).toBe('')
  })

  it('reports active durable persistence work', () => {
    vi.mocked(useServerHealth).mockReturnValue({
      data: ref({ status: 'ok', version: '0.1.0', persistence: { state: 'pending' } }),
    } as ReturnType<typeof useServerHealth>)
    const wrapper = mount(PersistenceStatus)
    expect(wrapper.text()).toContain('Saving model')
  })
})