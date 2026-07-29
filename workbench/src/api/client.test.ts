import { afterEach, expect, it, vi } from 'vitest'

import { api, ApiError } from './client'

afterEach(() => vi.unstubAllGlobals())

it('normalizes one advice string into one complete line', async () => {
  vi.stubGlobal(
    'fetch',
    vi.fn().mockResolvedValue({
      ok: false,
      status: 422,
      json: async () => ({ message: 'The design will not solve.', advice: 'Fix the named field.' }),
    }),
  )

  const failure = await api.design('broken').catch((error: unknown) => error)

  expect(failure).toBeInstanceOf(ApiError)
  if (!(failure instanceof ApiError)) throw new Error('expected an API error')
  expect(failure.advice).toEqual(['Fix the named field.'])
})