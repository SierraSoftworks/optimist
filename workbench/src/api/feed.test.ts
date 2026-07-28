import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { watchDesign } from './feed'

/**
 * A socket that records itself rather than connecting.
 *
 * The point of these tests is how many connections the feed opens and when it
 * closes them, which is exactly what a real socket would hide behind timing.
 */
class FakeSocket {
  static opened: FakeSocket[] = []

  onopen: (() => void) | null = null
  onmessage: ((event: { data: string }) => void) | null = null
  onclose: (() => void) | null = null
  onerror: (() => void) | null = null
  closed = false
  readonly url: string

  constructor(url: string) {
    this.url = url
    FakeSocket.opened.push(this)
  }

  close() {
    this.closed = true
    this.onclose?.()
  }

  open() {
    this.onopen?.()
  }

  deliver(message: unknown) {
    this.onmessage?.({ data: JSON.stringify(message) })
  }
}

const live = () => FakeSocket.opened.filter((socket) => !socket.closed)

beforeEach(() => {
  FakeSocket.opened = []
  vi.stubGlobal('WebSocket', FakeSocket)
  vi.useFakeTimers()
})

afterEach(() => {
  // Feeds are module state that outlives one test by design, so the linger is
  // run out here. Without it the next test joins the previous one's connection
  // and measures nothing.
  vi.advanceTimersByTime(30_000)
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('watchDesign', () => {
  /**
   * The header and the view both watch the design in front of them.
   *
   * Before they shared a connection each opened its own, so every design was
   * subscribed to twice, every reconnect happened twice, and a browser reported
   * connections interrupted as the page loaded.
   */
  it('opens one socket however many watchers a design has', () => {
    const header = watchDesign('checkout', { onMessage: vi.fn() })
    const view = watchDesign('checkout', { onMessage: vi.fn() })

    expect(FakeSocket.opened).toHaveLength(1)
    header()
    view()
  })

  it('tells every watcher what arrives', () => {
    const header = vi.fn()
    const view = vi.fn()
    const stopHeader = watchDesign('checkout', { onMessage: header })
    const stopView = watchDesign('checkout', { onMessage: view })

    FakeSocket.opened[0].deliver({ type: 'lagged', missed: 2 })
    expect(header).toHaveBeenCalledWith({ type: 'lagged', missed: 2 })
    expect(view).toHaveBeenCalledWith({ type: 'lagged', missed: 2 })

    stopHeader()
    stopView()
  })

  it('keeps designs apart', () => {
    const stopOne = watchDesign('checkout', { onMessage: vi.fn() })
    const stopTwo = watchDesign('billing', { onMessage: vi.fn() })
    expect(FakeSocket.opened).toHaveLength(2)
    stopOne()
    stopTwo()
  })

  /**
   * Moving between views unmounts one watcher and mounts another.
   *
   * Closing on the first and reopening on the second costs a round trip and a
   * fresh snapshot to reach the state the connection already had.
   */
  it('survives a watcher leaving as another arrives', () => {
    const leaving = watchDesign('checkout', { onMessage: vi.fn() })
    leaving()
    const arriving = watchDesign('checkout', { onMessage: vi.fn() })

    vi.advanceTimersByTime(5_000)
    expect(FakeSocket.opened).toHaveLength(1)
    expect(live()).toHaveLength(1)
    arriving()
  })

  it('closes once the last watcher has gone and stayed gone', () => {
    const stop = watchDesign('checkout', { onMessage: vi.fn() })
    stop()

    expect(live()).toHaveLength(1)
    vi.advanceTimersByTime(5_000)
    expect(live()).toHaveLength(0)
  })

  /** A feed nobody is watching must not keep reconnecting to itself. */
  it('stops reconnecting once it has been closed deliberately', () => {
    const stop = watchDesign('checkout', { onMessage: vi.fn() })
    FakeSocket.opened[0].open()
    stop()
    vi.advanceTimersByTime(60_000)

    expect(FakeSocket.opened).toHaveLength(1)
  })

  it('reconnects with a backoff when the server drops it', () => {
    const stop = watchDesign('checkout', { onMessage: vi.fn() })
    FakeSocket.opened[0].open()
    FakeSocket.opened[0].close()

    expect(FakeSocket.opened).toHaveLength(1)
    vi.advanceTimersByTime(500)
    expect(FakeSocket.opened).toHaveLength(2)
    stop()
  })

  /** A watcher joining a live feed must not be told the feed is closed. */
  it('tells a late watcher the state it is joining', () => {
    const first = watchDesign('checkout', { onMessage: vi.fn() })
    FakeSocket.opened[0].open()

    const status = vi.fn()
    const second = watchDesign('checkout', { onMessage: vi.fn(), onStatusChange: status })
    expect(status).toHaveBeenCalledWith('open')

    first()
    second()
  })
})
