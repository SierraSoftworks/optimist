import { transport } from './transport'
import type { FeedConnection } from './transport'
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
  connection: FeedConnection | null
  watchers: Set<FeedHandlers>
  status: FeedStatus
  retry: number
  reconnect: ReturnType<typeof setTimeout> | null
  linger: ReturnType<typeof setTimeout> | null
  stopped: boolean
  /**
   * Which attempt is the live one.
   *
   * A connection that has been replaced or shut down can still report itself,
   * and acting on that would reconnect a feed nobody is watching.
   */
  generation: number
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
  const generation = ++feed.generation
  const live = () => feed.generation === generation && !feed.stopped && !leaving
  announce(feed, 'connecting')

  feed.connection = transport.connect(design, {
    onOpen: () => {
      if (!live()) return
      feed.retry = FIRST_RETRY_MS
      announce(feed, 'open')
    },

    onMessage: (message) => {
      if (!live()) return
      // Over a copy, because being told is what makes a watcher leave.
      for (const watcher of [...feed.watchers]) watcher.onMessage(message)
    },

    onClose: () => {
      if (!live()) return
      feed.connection = null
      announce(feed, 'closed')
      feed.reconnect = setTimeout(() => connect(design, feed), feed.retry)
      feed.retry = Math.min(feed.retry * 2, MAX_RETRY_MS)
    },
  })
}

function shutdown(feed: Feed) {
  feed.stopped = true
  feed.generation += 1
  if (feed.reconnect) clearTimeout(feed.reconnect)
  if (feed.linger) clearTimeout(feed.linger)
  feed.reconnect = null
  feed.linger = null
  const connection = feed.connection
  feed.connection = null
  connection?.close()
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
      connection: null,
      watchers: new Set(),
      status: 'closed',
      retry: FIRST_RETRY_MS,
      reconnect: null,
      linger: null,
      stopped: false,
      generation: 0,
    }
    feeds.set(design, feed)
  }

  const joined = feed
  if (joined.linger) {
    clearTimeout(joined.linger)
    joined.linger = null
  }
  joined.watchers.add(handlers)

  if (joined.connection === null && joined.reconnect === null) {
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
