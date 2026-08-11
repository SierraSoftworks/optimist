import { Channel, invoke } from '@tauri-apps/api/core'

import { ApiError, adviceLines } from '../errors'
import type { FeedMessage } from '../types'
import type { FeedConnection, FeedListener, Imported, Transport } from './contract'
import { designNamed, stored } from './transfer'

/** An archive somebody chose, named by where it is rather than by its bytes. */
interface Chosen {
  name: string
  path: string
}

/**
 * Rust refuses in the shape the server would have refused in.
 *
 * A command that fails for a reason IPC itself invented has no status to
 * report, and is treated as a fault in this process rather than a design the
 * person can do something about.
 */
function refused(error: unknown): ApiError {
  const failure = error as { status?: number; message?: string; advice?: unknown }
  if (typeof failure?.message !== 'string') return new ApiError(500, String(error), [])
  return new ApiError(failure.status ?? 500, failure.message, adviceLines(failure.advice))
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args)
  } catch (error) {
    throw refused(error)
  }
}

/** The workbench inside the desktop application, with no server to talk to. */
export const tauri: Transport = {
  request<T>(method: string, path: string, body?: unknown): Promise<T> {
    return call<T>('api_call', { method, path: `/api/v1${path}`, body: body ?? null })
  },

  connect(design: string, listener: FeedListener): FeedConnection {
    const channel = new Channel<string>()
    channel.onmessage = (raw) => {
      let message: FeedMessage
      try {
        message = JSON.parse(raw) as FeedMessage
      } catch {
        return
      }
      listener.onMessage(message)
    }

    let subscription: number | null = null
    let closed = false

    void call<number>('feed_subscribe', { design, channel })
      .then((id) => {
        // Nothing is listening any more, so the subscription is ended rather
        // than left feeding a channel whose messages go nowhere.
        if (closed) return void invoke('feed_unsubscribe', { id })
        subscription = id
        listener.onOpen()
      })
      .catch(() => listener.onClose())

    return {
      close() {
        closed = true
        if (subscription !== null) void invoke('feed_unsubscribe', { id: subscription })
      },
    }
  },

  async exportDesign(design: string): Promise<void> {
    await call('export_design', { design })
  },

  async importDesign(): Promise<Imported | null> {
    const chosen = await call<Chosen | null>('choose_archive')
    if (!chosen) return null
    const design = designNamed(chosen.name)
    // Only the path crosses the boundary: the file is read where it is stored.
    return stored(design, (replace) =>
      call('import_design', { path: chosen.path, design, replace }),
    )
  },

  workspace: {
    current: () => call<string>('workspace_folder'),
    choose: () => call<string | null>('choose_workspace'),
  },
}
