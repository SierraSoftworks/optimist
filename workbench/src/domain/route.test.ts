import { describe, expect, it } from 'vitest'
import { parseRoute, routePath } from './route'

describe('parseRoute', () => {
  it('reads the project and the view a link names', () => {
    expect(parseRoute('/projects/platform/feedback')).toEqual({
      projectId: 'platform',
      mode: 'feedback',
    })
  })

  it('opens a project on the default view when the link names none', () => {
    expect(parseRoute('/projects/B')).toEqual({ projectId: 'B', mode: 'explore' })
    expect(parseRoute('/projects/B/')).toEqual({ projectId: 'B', mode: 'explore' })
  })

  it('falls back to the default view rather than failing on an unknown one', () => {
    expect(parseRoute('/projects/B/telemetry')).toEqual({ projectId: 'B', mode: 'explore' })
  })

  it('names no project for a path outside the pattern', () => {
    for (const path of [
      '/',
      '/projects',
      '/scenarios/B/explore',
      '/projects//explore',
      '/projects/B/explore/nodes',
    ]) {
      expect(parseRoute(path).projectId).toBeNull()
    }
  })

  /**
   * The project ID is interpolated straight into API paths, so a value the
   * server would reject must not reach them from a pasted link.
   */
  it('rejects project identifiers the server would not accept', () => {
    for (const id of ['has space', 'has-dash', 'a%2Fb', '%2e%2e', 'x'.repeat(65), '..', '.']) {
      expect(parseRoute(`/projects/${id}/explore`).projectId).toBeNull()
    }
  })

  it('survives an escape it cannot decode', () => {
    expect(parseRoute('/projects/%E0%A4%A/explore').projectId).toBeNull()
  })
})

describe('routePath', () => {
  it('renders the canonical address for a workbench state', () => {
    expect(routePath({ projectId: 'B', mode: 'optimize' })).toBe('/projects/B/optimize')
  })

  it('addresses the bare workbench when no project is open', () => {
    expect(routePath({ projectId: null, mode: 'feedback' })).toBe('/')
  })

  it('round-trips every view', () => {
    for (const mode of ['explore', 'impediments', 'feedback', 'optimize'] as const) {
      expect(parseRoute(routePath({ projectId: 'B', mode }))).toEqual({ projectId: 'B', mode })
    }
  })
})
