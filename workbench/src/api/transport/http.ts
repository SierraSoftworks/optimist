import { ApiError, adviceLines } from '../errors'
import type { FeedMessage } from '../types'
import type { FeedConnection, FeedListener, Imported, Transport } from './contract'
import { designNamed, stored } from './transfer'

/** Where a design's archive is fetched from, and where one is sent. */
function archive(design: string): string {
  return `/api/v1/designs/${encodeURIComponent(design)}/archive`
}

async function refusal(response: Response): Promise<ApiError> {
  // A refusal is expected to be JSON, but a proxy or a crash can produce
  // something else, and the status is worth reporting either way.
  const failure = await response.json().catch(() => null)
  return new ApiError(
    response.status,
    failure?.message ?? `The request failed with status ${response.status}.`,
    adviceLines(failure?.advice),
  )
}

/** The workbench as a page, talking to the server that served it. */
export const http: Transport = {
  async request<T>(method: string, path: string, body?: unknown): Promise<T> {
    const response = await fetch(`/api/v1${path}`, {
      method,
      body: body === undefined ? undefined : JSON.stringify(body),
      headers: { Accept: 'application/json', 'Content-Type': 'application/json' },
    })
    if (!response.ok) throw await refusal(response)
    // A deletion succeeds with no body, and asking for JSON that was never sent
    // would turn it into a failure.
    return response.status === 204 ? (undefined as T) : ((await response.json()) as T)
  },

  connect(design: string, listener: FeedListener): FeedConnection {
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
    const socket = new WebSocket(
      `${protocol}//${location.host}/api/v1/designs/${encodeURIComponent(design)}/feed`,
    )

    socket.onopen = () => listener.onOpen()

    socket.onmessage = (event) => {
      // A message this client cannot parse is one the server should not have
      // sent. Dropping it keeps the feed alive for the messages that follow.
      let message: FeedMessage
      try {
        message = JSON.parse(event.data as string) as FeedMessage
      } catch {
        return
      }
      listener.onMessage(message)
    }

    socket.onclose = () => listener.onClose()

    // An error is always followed by a close, so recovery is handled there
    // rather than in both places.
    socket.onerror = () => socket.close()

    return {
      close() {
        // Detached first: this close is deliberate, and the caller would
        // otherwise be told the feed dropped and reconnect to it.
        socket.onclose = null
        socket.onerror = null
        socket.close()
      },
    }
  },

  /**
   * Hands the archive to the browser rather than fetching it here.
   *
   * The server already says what the file is called and that it is a download,
   * so it streams to disk rather than being assembled in this tab first.
   */
  async exportDesign(design: string): Promise<void> {
    const link = document.createElement('a')
    link.href = archive(design)
    link.download = `${design}.zip`
    link.click()
  },

  async importDesign(): Promise<Imported | null> {
    const file = await chosen()
    if (!file) return null
    const design = designNamed(file.name)
    return stored(design, (replace) => put(design, file, replace))
  },
}

/**
 * Asks for a file the only way a browser offers to.
 *
 * The input is made here and thrown away again rather than living in the page,
 * because it is a way of asking a question and not part of what the page shows.
 * A picker that is dismissed resolves with nothing, and one that is dismissed
 * without saying so leaves this pending, which is the same as nothing having
 * happened.
 */
function chosen(): Promise<File | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input')
    input.type = 'file'
    input.accept = '.zip,application/zip'
    input.hidden = true
    // Attached because some browsers do not fire `change` on a detached input.
    document.body.append(input)

    const answer = (file: File | null) => {
      input.remove()
      resolve(file)
    }
    input.addEventListener('change', () => answer(input.files?.[0] ?? null))
    input.addEventListener('cancel', () => answer(null))
    input.click()
  })
}

async function put(design: string, contents: Blob, replace: boolean): Promise<void> {
  const response = await fetch(`${archive(design)}${replace ? '?replace=true' : ''}`, {
    method: 'PUT',
    body: contents,
    headers: { Accept: 'application/json', 'Content-Type': 'application/zip' },
  })
  if (!response.ok) throw await refusal(response)
}
