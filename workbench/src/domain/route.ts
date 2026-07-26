import type { WorkbenchMode } from '../stores/workbench'

/**
 * The part of the workbench's state that belongs in the address bar.
 *
 * Which project is open and which view is showing survive a reload, a bookmark,
 * and a link pasted into a chat; everything else the workbench holds -- the
 * selected node, the search text, the kind filters -- is scratch state that
 * would only make a shared link harder to read.
 *
 * ```ts
 * import { parseRoute, routePath } from './route'
 *
 * const route = parseRoute('/projects/platform/feedback')
 * // { projectId: 'platform', mode: 'feedback' }
 * routePath(route) // '/projects/platform/feedback'
 * ```
 */
export interface WorkbenchRoute {
  /** Project to open, or `null` when the address bar names none. */
  projectId: string | null
  /** View to show; unknown and missing views resolve to `explore`. */
  mode: WorkbenchMode
}

const MODES: readonly string[] = ['explore', 'impediments', 'feedback', 'optimize']

/**
 * The server's own project ID rule: 1-64 ASCII alphanumerics, `_`, and `.`.
 *
 * The workbench interpolates this value straight into API paths, and it now
 * arrives from whatever a visitor typed rather than only from the project list,
 * so it is checked against the server's grammar before it is trusted. Bare `.`
 * and `..` pass that grammar but would resolve as relative path segments, so
 * they are rejected here as well.
 */
const PROJECT_ID = /^[A-Za-z0-9_.]{1,64}$/

const UNROUTED: WorkbenchRoute = { projectId: null, mode: 'explore' }

/**
 * Reads `/projects/{project}/{view}` into the state it names.
 *
 * Anything the pattern does not cover -- a different prefix, a project ID the
 * server would reject, an undecodable escape -- resolves to no project rather
 * than to an error, so a mistyped link lands on the workbench instead of on a
 * failure page.
 */
export function parseRoute(pathname: string): WorkbenchRoute {
  const parts = pathname.split('/')
  if (parts.length > 3 && parts[parts.length - 1] === '') parts.pop()
  const [root, prefix, project, view] = parts.map(segment)
  if (root !== '' || prefix !== 'projects' || parts.length > 4) return UNROUTED
  if (!project || !PROJECT_ID.test(project) || !/[A-Za-z0-9_]/.test(project)) return UNROUTED
  return { projectId: project, mode: MODES.includes(view ?? '') ? (view as WorkbenchMode) : 'explore' }
}

/** Renders the canonical address for one workbench state. */
export function routePath(route: WorkbenchRoute): string {
  if (!route.projectId) return '/'
  return `/projects/${encodeURIComponent(route.projectId)}/${route.mode}`
}

function segment(value: string): string {
  try {
    return decodeURIComponent(value)
  } catch {
    return ''
  }
}
