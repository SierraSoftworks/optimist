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
 *
 * # One socket per design, however many watchers
 *
 * Several parts of the page want to know when a design changes: the header
 * shows its name, the view shows its contents. Each opening its own socket meant
 * a design was subscribed to two and three times over, which multiplied every
 * reconnect and left the browser reporting connections interrupted as the page
 * loaded. Watchers therefore share one connection and are counted, so the socket
 * opens with the first and closes after the last.
 *
 * Closing is delayed by a moment. Moving between the design and review views
 * unmounts one watcher and mounts another at almost the same instant, and a
 * connection torn down across that gap costs a round trip and a fresh snapshot
 * to arrive back at the state it already had.
 */
export interface FeedHandlers {
  onMessage: (message: FeedMessage) => void
  onStatusChange?: (status: FeedStatus) => void
}

export type FeedStatus = 'connecting' | 'open' | 'closed'

const FIRST_RETRY_MS = 500
const MAX_RETRY_MS = 10_000

/** How long a feed stays open after its last watcher has gone. */
const LINGER_MS = 2_000

interface Feed {
  socket: WebSocket | null
  watchers: Set<FeedHandlers>
  status: FeedStatus
  retry: number
  reconnect: ReturnType<typeof setTimeout> | null
  linger: ReturnType<typeof setTimeout> | null
  stopped: boolean
}

const feeds = new Map<string, Feed>()

/**
 * Whether the page is going away.
 *
 * A socket closed by navigation is not a feed that dropped, and reconnecting
 * into a page being torn down is what the browser reports as a connection
 * interrupted while the page was loading.
 */
let leaving = false
if (typeof window !== 'undefined') {
  window.addEventListener('pagehide', () => {
    leaving = true
    for (const feed of feeds.values()) shutdown(feed)
    feeds.clear()
  })
}

function announce(feed: Feed, status: FeedStatus) {
  feed.status = status
  for (const watcher of [...feed.watchers]) watcher.onStatusChange?.(status)
}

function connect(design: string, feed: Feed) {
  feed.reconnect = null
  if (feed.stopped || leaving) return
  announce(feed, 'connecting')
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
  const socket = new WebSocket(
    `${protocol}//${location.host}/api/v1/designs/${encodeURIComponent(design)}/feed`,
  )
  feed.socket = socket

  socket.onopen = () => {
    feed.retry = FIRST_RETRY_MS
    announce(feed, 'open')
  }

  socket.onmessage = (event) => {
    // A message this client cannot parse is one the server should not have
    // sent. Dropping it keeps the feed alive for the messages that follow.
    let message: FeedMessage
    try {
      message = JSON.parse(event.data as string) as FeedMessage
    } catch {
      return
    }
    // Over a copy, because being told is what makes a watcher leave.
    for (const watcher of [...feed.watchers]) watcher.onMessage(message)
  }

  socket.onclose = () => {
    if (feed.socket !== socket) return
    feed.socket = null
    announce(feed, 'closed')
    if (feed.stopped || leaving) return
    feed.reconnect = setTimeout(() => connect(design, feed), feed.retry)
    feed.retry = Math.min(feed.retry * 2, MAX_RETRY_MS)
  }

  // An error is always followed by a close, so recovery is handled there
  // rather than in both places.
  socket.onerror = () => socket.close()
}

function shutdown(feed: Feed) {
  feed.stopped = true
  if (feed.reconnect) clearTimeout(feed.reconnect)
  if (feed.linger) clearTimeout(feed.linger)
  feed.reconnect = null
  feed.linger = null
  const socket = feed.socket
  feed.socket = null
  if (socket) {
    // Detached first: this close is deliberate, and the handler would otherwise
    // schedule a reconnect for a feed nobody is watching.
    socket.onclose = null
    socket.onerror = null
    socket.close()
  }
}

/**
 * Watches a design, joining the connection if one is already open.
 *
 * The returned function stops this watcher. The socket outlives it by a moment,
 * in case something else is about to ask for the same design.
 */
export function watchDesign(design: string, handlers: FeedHandlers): () => void {
  let feed = feeds.get(design)
  if (!feed || feed.stopped) {
    feed = {
      socket: null,
      watchers: new Set(),
      status: 'closed',
      retry: FIRST_RETRY_MS,
      reconnect: null,
      linger: null,
      stopped: false,
    }
    feeds.set(design, feed)
  }

  const joined = feed
  if (joined.linger) {
    clearTimeout(joined.linger)
    joined.linger = null
  }
  joined.watchers.add(handlers)

  if (joined.socket === null && joined.reconnect === null) {
    connect(design, joined)
  } else {
    // A watcher that arrives after the socket opened would otherwise never be
    // told what state it is in, and would report a live feed as closed.
    handlers.onStatusChange?.(joined.status)
  }

  return () => {
    joined.watchers.delete(handlers)
    if (joined.watchers.size > 0) return
    joined.linger = setTimeout(() => {
      if (joined.watchers.size > 0) return
      shutdown(joined)
      if (feeds.get(design) === joined) feeds.delete(design)
    }, LINGER_MS)
  }
}
