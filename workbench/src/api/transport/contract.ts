import type { FeedMessage } from '../types'

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

/** What became of an archive somebody chose. */
export type Imported =
  | { status: 'stored'; design: string }
  | {
      status: 'conflict'
      design: string
      /** Sends the same archive again, over the design already there. */
      replace: () => Promise<Imported>
    }

/** Where designs are kept, on hosts where that is a person's to decide. */
export interface WorkspaceFolder {
  /** The folder currently open. */
  current: () => Promise<string>
  /**
   * Asks for another and opens it there and then.
   *
   * Resolves with nothing when the person changes their mind. Designs in the
   * folder that was open are not designs in the one that is, so a caller must
   * treat everything it was holding as gone.
   */
  choose: () => Promise<string | null>
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
  /**
   * Puts a design's archive somewhere the person asking for it can find it.
   *
   * A browser and a window disagree about what that means and about how a file
   * is chosen, so each owns the whole exchange rather than handing bytes back
   * to be moved by code that would then have to know which host it was in.
   */
  exportDesign: (design: string) => Promise<void>
  /**
   * Asks for an archive and stores it under the name its file suggests.
   *
   * Resolves with nothing when the person changes their mind, which is not a
   * failure and has nothing to report.
   */
  importDesign: () => Promise<Imported | null>
  /**
   * Absent in a browser, where the folder belongs to whoever ran the server
   * and is not this page's to change.
   */
  workspace?: WorkspaceFolder
}
