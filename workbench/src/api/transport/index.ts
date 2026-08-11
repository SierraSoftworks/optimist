import type { Transport } from './contract'
import { http } from './http'
import { tauri } from './tauri'

export type {
  FeedConnection,
  FeedListener,
  Imported,
  Transport,
  WorkspaceFolder,
} from './contract'

/**
 * Which one is in front of us.
 *
 * Tauri puts this on the window before any of our code runs, so the choice is
 * made once at load rather than asked again on every call.
 */
const desktop = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

export const transport: Transport = desktop ? tauri : http
