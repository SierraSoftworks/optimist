import { VueQueryPlugin } from '@tanstack/vue-query'
import { mount } from '@vue/test-utils'
import ElementPlus from 'element-plus'
import { afterEach, expect, it, vi } from 'vitest'

import { ApiError, api } from '../api/client'
import type { Imported } from '../api/transport'
import DesignTransfer from './DesignTransfer.vue'

afterEach(() => {
  vi.restoreAllMocks()
  document.body.innerHTML = ''
})

function transfer() {
  return mount(DesignTransfer, {
    props: { design: 'checkout' },
    attachTo: document.body,
    global: { plugins: [ElementPlus, VueQueryPlugin] },
  })
}

/** Lets promises settle and the dialogs they open reach the document. */
async function settle(wrapper: { vm: { $nextTick: () => Promise<void> } }): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve))
  await wrapper.vm.$nextTick()
}

async function importing(wrapper: ReturnType<typeof transfer>): Promise<void> {
  await wrapper.get('[data-test="import-design"]').trigger('click')
  await settle(wrapper)
}

it('reports the design an archive was stored as', async () => {
  vi.spyOn(api, 'importDesign').mockResolvedValue({
    status: 'stored',
    design: 'payments-ledger',
  })
  const wrapper = transfer()

  await importing(wrapper)

  expect(wrapper.emitted('imported')).toEqual([['payments-ledger']])
})

/** Changing your mind about a file is not something to be told about. */
it('says nothing when no archive was chosen', async () => {
  vi.spyOn(api, 'importDesign').mockResolvedValue(null)
  const wrapper = transfer()

  await importing(wrapper)

  expect(wrapper.emitted('imported')).toBeUndefined()
  expect(document.body.textContent).not.toContain('Replace this design?')
})

it('asks before replacing a design, and only then sends it again', async () => {
  const replace = vi.fn<() => Promise<Imported>>().mockResolvedValue({
    status: 'stored',
    design: 'checkout',
  })
  vi.spyOn(api, 'importDesign').mockResolvedValue({
    status: 'conflict',
    design: 'checkout',
    replace,
  })
  const wrapper = transfer()

  await importing(wrapper)

  expect(wrapper.emitted('imported')).toBeUndefined()
  expect(document.body.textContent).toContain('Replace this design?')

  await wrapper.get('[data-test="import-replace"]').trigger('click')
  await settle(wrapper)

  expect(replace).toHaveBeenCalled()
  expect(wrapper.emitted('imported')).toEqual([['checkout']])
})

it('shows what was wrong with a refused archive and what to do about it', async () => {
  vi.spyOn(api, 'importDesign').mockRejectedValue(
    new ApiError(422, 'this file is not a readable archive', [
      'Check the file downloaded completely, then try again.',
    ]),
  )
  const wrapper = transfer()

  await importing(wrapper)

  expect(document.body.textContent).toContain('this file is not a readable archive')
  expect(document.body.textContent).toContain('Check the file downloaded completely')
  expect(wrapper.emitted('imported')).toBeUndefined()
})

it('reports an export that could not be written', async () => {
  vi.spyOn(api, 'exportDesign').mockRejectedValue(
    new ApiError(500, 'the folder could not be written to', ['Choose somewhere else.']),
  )
  const wrapper = transfer()

  await wrapper.get('[data-test="export-design"]').trigger('click')
  await settle(wrapper)

  expect(document.body.textContent).toContain('the folder could not be written to')
})
