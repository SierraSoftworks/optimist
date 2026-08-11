import { ApiError } from './errors'
import { transport } from './transport'
import type {
  Analysis,
  Applied,
  Catalogue,
  Comparison,
  DesignSummary,
  Mutation,
  Quantity,
  Snapshot,
} from './types'

export { ApiError }

/** Controls a caller may vary without editing the design. */
export interface SolveControls {
  seed?: number
  samples?: number
  horizon?: number
  step?: number
  intervention?: string | null
  /** Ask for every step, not only the one the design settled on. */
  series?: boolean
  /**
   * Walk the design through time rather than solving for where it balances.
   *
   * Only this shows how long an incident outlasts its cause, because only this
   * makes the queues fill and drain at a finite rate. Considerably slower, and
   * wants a shorter step and a longer horizon to go with it.
   */
  transient?: boolean
}

function query(controls: SolveControls): string {
  const parameters = new URLSearchParams()
  if (controls.seed !== undefined) parameters.set('seed', String(controls.seed))
  if (controls.samples !== undefined) parameters.set('samples', String(controls.samples))
  if (controls.horizon !== undefined) parameters.set('horizon', String(controls.horizon))
  if (controls.step !== undefined) parameters.set('step', String(controls.step))
  if (controls.intervention) parameters.set('intervention', controls.intervention)
  if (controls.series) parameters.set('series', 'true')
  if (controls.transient) parameters.set('transient', 'true')
  const rendered = parameters.toString()
  return rendered ? `?${rendered}` : ''
}

function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  return transport.request<T>(method, path, body)
}

export const api = {
  designs: () => request<DesignSummary[]>('GET', '/designs'),

  create: (id: string, name: string, summary: string) =>
    request<Snapshot>('POST', '/designs', { id, name, summary }),

  design: (design: string) => request<Snapshot>('GET', `/designs/${encodeURIComponent(design)}`),

  /** Deletes a design and everything stored under it. */
  remove: (design: string) =>
    request<void>('DELETE', `/designs/${encodeURIComponent(design)}`),

  catalogue: (design: string) =>
    request<Catalogue>('GET', `/designs/${encodeURIComponent(design)}/catalogue`),

  /** Puts the design's archive somewhere the person asking for it can find it. */
  exportDesign: (design: string) => transport.exportDesign(design),

  /**
   * Asks for an archive and stores it under the name its file suggests.
   *
   * Resolves with nothing when the person changes their mind, and reports a
   * design of that name already existing as a conflict to put to them rather
   * than as a failure.
   */
  importDesign: () => transport.importDesign(),

  /**
   * The folder designs are kept in, where that is a person's to decide.
   *
   * Absent in a browser: the folder belongs to whoever ran the server, and a
   * page changing it would change it for everybody else looking at the same one.
   */
  workspace: transport.workspace,

  /**
   * Applies edits in order, stopping at the first that will not apply.
   *
   * Earlier edits in a rejected batch stand, because each names one entity and
   * is complete on its own. The returned count says how many landed.
   */
  mutate: (design: string, mutations: Mutation[]) =>
    request<Applied>('POST', `/designs/${encodeURIComponent(design)}/mutations`, { mutations }),

  analysis: (design: string, controls: SolveControls = {}) =>
    request<Analysis>('GET', `/designs/${encodeURIComponent(design)}/analysis${query(controls)}`),

  /**
   * Evaluates one expression the way the solver would, for a preview.
   *
   * `entry` names the shared quantity being edited, because a quantity sees only
   * the ones declared ahead of it and a preview that ignored that would show a
   * figure the solver is going to refuse.
   */
  preview: (design: string, expression: string, entry: string | null = null) =>
    request<Quantity>('POST', `/designs/${encodeURIComponent(design)}/preview`, {
      expression,
      entry,
    }),

  comparison: (design: string, intervention: string, controls: SolveControls = {}) =>
    request<Comparison>(
      'GET',
      `/designs/${encodeURIComponent(design)}/comparisons/${encodeURIComponent(intervention)}${query(
        controls,
      )}`,
    ),
}
