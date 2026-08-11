import { describe, expect, it, vi } from 'vitest'

import { ApiError } from '../errors'
import { designNamed, stored } from './transfer'

describe('designNamed', () => {
  it.each([
    ['Payments Ledger.ZIP', 'payments-ledger'],
    ['checkout.zip', 'checkout'],
    ['--odd__name--.zip', 'odd-name'],
  ])('names the design after %s', (file, design) => {
    expect(designNamed(file)).toBe(design)
  })

  /** A file that cannot name a design is a question, not a silent failure. */
  it('refuses a name with nothing in it to use', () => {
    expect(() => designNamed('....zip')).toThrow(ApiError)
  })
})

describe('stored', () => {
  it('reports an archive that landed', async () => {
    const put = vi.fn().mockResolvedValue(undefined)

    await expect(stored('checkout', put)).resolves.toEqual({
      status: 'stored',
      design: 'checkout',
    })
    expect(put).toHaveBeenCalledWith(false)
  })

  /**
   * A design already being there is a question rather than a failure.
   *
   * The retry sends the same archive, so the person is asked once and never
   * asked to find the file again.
   */
  it('turns a refusal to overwrite into something to answer', async () => {
    const put = vi
      .fn()
      .mockRejectedValueOnce(new ApiError(409, 'already here', []))
      .mockResolvedValueOnce(undefined)

    const result = await stored('checkout', put)
    expect(result.status).toBe('conflict')
    if (result.status !== 'conflict') return

    await expect(result.replace()).resolves.toEqual({
      status: 'stored',
      design: 'checkout',
    })
    expect(put).toHaveBeenLastCalledWith(true)
  })

  it('lets every other refusal through', async () => {
    const put = vi.fn().mockRejectedValue(new ApiError(422, 'not an archive', []))

    await expect(stored('checkout', put)).rejects.toThrow('not an archive')
  })
})
