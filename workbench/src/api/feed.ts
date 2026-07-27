import type { FeedMessage } from './types'

/**
 * A design's change feed.
 *
 * The socket carries other people's edits, which are applied to the local copy
 * rather than triggering a refetch. Refetching would replace the whole design,
 * discarding whatever the person at this keyboard was in the middle of typing.
 *
 * Reconnection backs off, because the common reason a feed drops is that the
 * server is restarting and a client that retries in a tight loop delays the
 * moment it comes back.
 */
export interface FeedHandlers {
  onMessage: (message: FeedMessage) => void
  onStatusChange?: (status: FeedStatus) => void
}

export type FeedStatus = 'connecting' | 'open' | 'closed'

const FIRST_RETRY_MS = 500
const MAX_RETRY_MS = 10_000

export function watchDesign(design: string, handlers: FeedHandlers): () => void {
  let socket: WebSocket | null = null
  let retry = FIRST_RETRY_MS
  let timer: ReturnType<typeof setTimeout> | null = null
  let stopped = false

  const setStatus = (status: FeedStatus) => handlers.onStatusChange?.(status)

  const connect = () => {
    if (stopped) return
    setStatus('connecting')
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
    const url = `${protocol}//${location.host}/api/v1/designs/${encodeURIComponent(design)}/feed`
    socket = new WebSocket(url)

    socket.onopen = () => {
      retry = FIRST_RETRY_MS
      setStatus('open')
    }

    socket.onmessage = (event) => {
      // A message this client cannot parse is one the server should not have
      // sent. Dropping it keeps the feed alive for the messages that follow.
      try {
        handlers.onMessage(JSON.parse(event.data as string) as FeedMessage)
      } catch {
        /* ignore */
      }
    }

    socket.onclose = () => {
      setStatus('closed')
      if (stopped) return
      timer = setTimeout(connect, retry)
      retry = Math.min(retry * 2, MAX_RETRY_MS)
    }

    // An error is always followed by a close, so recovery is handled there
    // rather than in both places.
    socket.onerror = () => socket?.close()
  }

  connect()

  return () => {
    stopped = true
    if (timer) clearTimeout(timer)
    socket?.close()
  }
}
