import type { FeedMessage, Snapshot } from '../types'

/** What a watcher of one design is told, whatever is carrying it. */
export interface FeedListener {
  onOpen: () => void
  onMessage: (message: FeedMessage) => void
  onClose: () => void
}

/** A subscription to one design. */
export interface FeedConnection {
  /** Ends it deliberately, which is not a drop and must not be retried. */
  close: () => void
}

/** An archive somebody chose, and what they called it. */
export interface Archive {
  name: string
  data: Blob
}

/**
 * How the workbench reaches the designs it is editing.
 *
 * The same bundle runs in a browser served by `optimist serve` and inside the
 * desktop application, which has no server to talk to and reaches Rust over
 * Tauri's IPC instead. Everything that differs between the two lives here, so
 * nothing above has to ask which one it is running in.
 */
export interface Transport {
  /** Sends one request and returns what came back, or throws `ApiError`. */
  request: <T>(method: string, path: string, body?: unknown) => Promise<T>
  /** Subscribes to a design's changes. Reconnection belongs to the caller. */
  connect: (design: string, listener: FeedListener) => FeedConnection
  /** Puts a design's archive somewhere the person asking for it can find it. */
  saveArchive: (design: string) => Promise<void>
  /** Stores an archive under a design, refusing to replace unless told to. */
  putArchive: (design: string, archive: Blob, replace: boolean) => Promise<Snapshot>
  /**
   * Asks for an archive with the platform's own picker.
   *
   * Absent in a browser, which has none to offer: choosing a file there means
   * an `<input>` in the document, and the component doing the importing already
   * owns one.
   */
  chooseArchive?: () => Promise<Archive | null>
}
