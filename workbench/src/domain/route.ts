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
  /**
   * Scenario the view is reading, where the view has one.
   *
   * Only the optimize view selects a scenario, so this is absent elsewhere. It
   * sits in the path rather than a query because a scenario belongs to a
   * project the same way a project belongs to the workbench.
   */
  scenarioId: string | null
  /**
   * Candidate intervention being read, which only means anything under a
   * scenario and so is only written alongside one.
   */
  candidateId: string | null
}

const MODES: readonly string[] = ['explore', 'impediments', 'feedback', 'optimize']

/**
 * The server's own project and entity ID rule: ASCII alphanumerics, `_`, and `.`.
 *
 * The workbench interpolates these values straight into API paths, and they now
 * arrive from whatever a visitor typed rather than only from the project list,
 * so they are checked against the server's grammar before they are trusted. Bare
 * `.` and `..` pass that grammar but would resolve as relative path segments, so
 * they are rejected here as well.
 */
const IDENTIFIER = /^[A-Za-z0-9_.]{1,64}$/

const UNROUTED: WorkbenchRoute = {
  projectId: null,
  mode: 'explore',
  scenarioId: null,
  candidateId: null,
}

/**
 * Reads `/projects/{project}/{view}/{scenario}/{candidate}` into the state it names.
 *
 * Anything the pattern does not cover -- a different prefix, an ID the server
 * would reject, an undecodable escape -- resolves to no project rather than to
 * an error, so a mistyped link lands on the workbench instead of on a failure
 * page. The trailing two segments are optional and positional: a candidate only
 * means anything under a scenario, so it is never read without one.
 */
export function parseRoute(pathname: string): WorkbenchRoute {
  const parts = pathname.split('/')
  if (parts.length > 3 && parts[parts.length - 1] === '') parts.pop()
  const [root, prefix, project, view, scenario, candidate] = parts.map(segment)
  if (root !== '' || prefix !== 'projects' || parts.length > 6) return UNROUTED
  if (!identifier(project)) return UNROUTED
  return {
    projectId: project!,
    mode: MODES.includes(view ?? '') ? (view as WorkbenchMode) : 'explore',
    scenarioId: identifier(scenario) ? scenario! : null,
    candidateId: identifier(scenario) && identifier(candidate) ? candidate! : null,
  }
}

/** Renders the canonical address for one workbench state. */
export function routePath(route: WorkbenchRoute): string {
  if (!route.projectId) return '/'
  const parts = ['projects', encodeURIComponent(route.projectId), route.mode]
  if (route.scenarioId) {
    parts.push(encodeURIComponent(route.scenarioId))
    if (route.candidateId) parts.push(encodeURIComponent(route.candidateId))
  }
  return `/${parts.join('/')}`
}

function identifier(value: string | undefined): boolean {
  return Boolean(value) && IDENTIFIER.test(value!) && /[A-Za-z0-9_]/.test(value!)
}

function segment(value: string): string {
  try {
    return decodeURIComponent(value)
  } catch {
    return ''
  }
}
