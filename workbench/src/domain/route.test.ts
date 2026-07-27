import { describe, expect, it } from 'vitest'
import { parseRoute, routePath } from './route'

describe('parseRoute', () => {
  it('reads the project and the view a link names', () => {
    expect(parseRoute('/projects/platform/feedback')).toEqual({
      projectId: 'platform',
      mode: 'feedback',
      scenarioId: null,
      candidateId: null,
    })
  })

  it('reads the scenario and candidate an optimize link names', () => {
    expect(parseRoute('/projects/B/optimize/A/K')).toEqual({
      projectId: 'B',
      mode: 'optimize',
      scenarioId: 'A',
      candidateId: 'K',
    })
  })

  it('reads a scenario without a candidate', () => {
    expect(parseRoute('/projects/B/optimize/A')).toEqual({
      projectId: 'B',
      mode: 'optimize',
      scenarioId: 'A',
      candidateId: null,
    })
  })

  /**
   * A candidate only means anything under a scenario, and the segments are
   * positional, so an unusable scenario takes the candidate with it.
   */
  it('ignores a candidate whose scenario segment is unusable', () => {
    const route = parseRoute('/projects/B/optimize/..%2F/K')
    expect(route.scenarioId).toBeNull()
    expect(route.candidateId).toBeNull()
  })

  it('opens a project on the default view when the link names none', () => {
    expect(parseRoute('/projects/B')).toEqual({
      projectId: 'B', mode: 'explore', scenarioId: null, candidateId: null,
    })
    expect(parseRoute('/projects/B/')).toEqual({
      projectId: 'B', mode: 'explore', scenarioId: null, candidateId: null,
    })
  })

  it('falls back to the default view rather than failing on an unknown one', () => {
    expect(parseRoute('/projects/B/telemetry')).toEqual({
      projectId: 'B', mode: 'explore', scenarioId: null, candidateId: null,
    })
  })

  it('names no project for a path outside the pattern', () => {
    for (const path of [
      '/',
      '/projects',
      '/scenarios/B/explore',
      '/projects//explore',
      '/projects/B/optimize/A/K/extra',
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
    expect(routePath({
      projectId: 'B', mode: 'optimize', scenarioId: null, candidateId: null,
    })).toBe('/projects/B/optimize')
  })

  it('names the scenario and candidate an optimize view is reading', () => {
    expect(routePath({
      projectId: 'B', mode: 'optimize', scenarioId: 'A', candidateId: 'K',
    })).toBe('/projects/B/optimize/A/K')
  })

  /** The segments are positional, so a candidate cannot be written alone. */
  it('omits a candidate that has no scenario to sit under', () => {
    expect(routePath({
      projectId: 'B', mode: 'optimize', scenarioId: null, candidateId: 'K',
    })).toBe('/projects/B/optimize')
  })

  it('addresses the bare workbench when no project is open', () => {
    expect(routePath({
      projectId: null, mode: 'feedback', scenarioId: null, candidateId: null,
    })).toBe('/')
  })

  it('round-trips every view', () => {
    for (const mode of ['explore', 'impediments', 'feedback', 'optimize'] as const) {
      const route = { projectId: 'B', mode, scenarioId: null, candidateId: null }
      expect(parseRoute(routePath(route))).toEqual(route)
    }
  })

  it('round-trips a full optimize selection', () => {
    const route = {
      projectId: 'B', mode: 'optimize' as const, scenarioId: 'A', candidateId: 'K',
    }
    expect(parseRoute(routePath(route))).toEqual(route)
  })
})
