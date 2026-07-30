import { VueQueryPlugin } from '@tanstack/vue-query'
import { mount } from '@vue/test-utils'
import ElementPlus from 'element-plus'
import { afterEach, expect, it, vi } from 'vitest'

import { ApiError, api } from '../api/client'
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

/** Drives the hidden file input the way choosing a file in the browser does. */
async function choose(
  wrapper: ReturnType<typeof transfer>,
  name: string,
): Promise<void> {
  const input = wrapper.get('[data-test="import-file"]').element as HTMLInputElement
  Object.defineProperty(input, 'files', {
    configurable: true,
    value: [new File(['archive'], name, { type: 'application/zip' })],
  })
  await wrapper.get('[data-test="import-file"]').trigger('change')
  await settle(wrapper)
}

it('names the design after the file it was chosen from', async () => {
  const importing = vi.spyOn(api, 'importArchive').mockResolvedValue({} as never)
  const wrapper = transfer()

  await choose(wrapper, 'Payments Ledger.ZIP')

  expect(importing).toHaveBeenCalledWith('payments-ledger', expect.anything(), false)
  expect(wrapper.emitted('imported')).toEqual([['payments-ledger']])
})

it('asks before replacing a design, and only then sends it again', async () => {
  const importing = vi
    .spyOn(api, 'importArchive')
    .mockRejectedValueOnce(new ApiError(409, 'A design named checkout already exists.', []))
    .mockResolvedValueOnce({} as never)
  const wrapper = transfer()

  await choose(wrapper, 'checkout.zip')

  expect(wrapper.emitted('imported')).toBeUndefined()
  expect(document.body.textContent).toContain('Replace this design?')

  await wrapper.get('[data-test="import-replace"]').trigger('click')
  await settle(wrapper)

  expect(importing).toHaveBeenLastCalledWith('checkout', expect.anything(), true)
  expect(wrapper.emitted('imported')).toEqual([['checkout']])
})

it('shows what was wrong with a refused archive and what to do about it', async () => {
  vi.spyOn(api, 'importArchive').mockRejectedValue(
    new ApiError(422, 'this file is not a readable archive', [
      'Check the file downloaded completely, then try again.',
    ]),
  )
  const wrapper = transfer()

  await choose(wrapper, 'broken.zip')

  expect(document.body.textContent).toContain('this file is not a readable archive')
  expect(document.body.textContent).toContain('Check the file downloaded completely')
  expect(wrapper.emitted('imported')).toBeUndefined()
})
