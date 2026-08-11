import { beforeEach, describe, expect, it, vi } from 'vitest'

import { ApiError } from '../errors'

class FakeChannel<T> {
  onmessage: ((message: T) => void) | null = null
}

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  Channel: FakeChannel,
}))

const { invoke } = await import('@tauri-apps/api/core')
const { tauri } = await import('./tauri')

const invoked = vi.mocked(invoke)

/** Lets the subscription promise settle the way an awaited command would. */
const settle = () => new Promise((resolve) => setTimeout(resolve))

/** The channel handed to the command, which stands in for the window. */
function subscribed(): FakeChannel<string> {
  const [, args] = invoked.mock.calls[0] as [string, { channel: FakeChannel<string> }]
  return args.channel
}

/** The refusal a call produced, rather than the value it did not return. */
async function refused(call: Promise<unknown>): Promise<ApiError> {
  return (await call.catch((error: unknown) => error)) as ApiError
}

beforeEach(() => {
  invoked.mockReset()
})

describe('request', () => {
  it.each([
    ['GET', '/designs', undefined],
    ['DELETE', '/designs/checkout', undefined],
    ['POST', '/designs/checkout/mutations', { mutations: [] }],
  ])('carries %s %s to the command', async (method, path, body) => {
    invoked.mockResolvedValue({ ok: true })

    await expect(tauri.request(method, path, body)).resolves.toEqual({ ok: true })
    expect(invoked).toHaveBeenCalledWith('api_call', {
      method,
      path: `/api/v1${path}`,
      body: body ?? null,
    })
  })

  /**
   * A refusal has to survive the crossing intact.
   *
   * The interface decides what to show from the status and repeats the advice,
   * so a refusal flattened into a string on the way through would leave it with
   * nothing to say beyond that something went wrong.
   */
  it('reports a refusal as the server would have', async () => {
    invoked.mockRejectedValue({
      status: 409,
      message: 'A design named checkout already exists.',
      advice: ['Open the existing design, or choose another identifier.'],
    })

    const failure = await refused(tauri.request('GET', '/designs/checkout'))

    expect(failure).toBeInstanceOf(ApiError)
    expect(failure.status).toBe(409)
    expect(failure.advice).toEqual(['Open the existing design, or choose another identifier.'])
  })

  it('treats a failure with nothing to say as a fault in this process', async () => {
    invoked.mockRejectedValue('the command panicked')

    const failure = await refused(tauri.request('GET', '/designs'))

    expect(failure).toBeInstanceOf(ApiError)
    expect(failure.status).toBe(500)
  })
})

describe('connect', () => {
  it('delivers what the channel carries, once the subscription is granted', async () => {
    invoked.mockResolvedValue(7)
    const onMessage = vi.fn()
    const onOpen = vi.fn()

    const connection = tauri.connect('checkout', { onOpen, onMessage, onClose: vi.fn() })
    await settle()

    expect(onOpen).toHaveBeenCalled()
    subscribed().onmessage?.(JSON.stringify({ type: 'lagged', missed: 2 }))
    expect(onMessage).toHaveBeenCalledWith({ type: 'lagged', missed: 2 })

    connection.close()
    expect(invoked).toHaveBeenLastCalledWith('feed_unsubscribe', { id: 7 })
  })

  /** A message this client cannot read must not take the feed down with it. */
  it('drops a message it cannot parse', async () => {
    invoked.mockResolvedValue(1)
    const onMessage = vi.fn()

    tauri.connect('checkout', { onOpen: vi.fn(), onMessage, onClose: vi.fn() })
    await settle()

    const channel = subscribed()
    expect(() => channel.onmessage?.('not json')).not.toThrow()
    expect(onMessage).not.toHaveBeenCalled()  })

  /**
   * Watchers come and go faster than a command round trip.
   *
   * A subscription granted after the last watcher left would otherwise be held
   * open for the rest of the session, feeding a channel nobody reads.
   */
  it('ends a subscription granted after it was already closed', async () => {
    invoked.mockResolvedValue(4)

    tauri.connect('checkout', { onOpen: vi.fn(), onMessage: vi.fn(), onClose: vi.fn() }).close()
    await settle()

    expect(invoked).toHaveBeenCalledWith('feed_unsubscribe', { id: 4 })
  })

  it('reports a design it cannot watch as a feed that closed', async () => {
    invoked.mockRejectedValue({ status: 404, message: 'no such design', advice: [] })
    const onClose = vi.fn()

    tauri.connect('missing', { onOpen: vi.fn(), onMessage: vi.fn(), onClose })
    await settle()

    expect(onClose).toHaveBeenCalled()
  })
})
