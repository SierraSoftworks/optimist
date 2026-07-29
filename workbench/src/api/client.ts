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

/**
 * A refusal from the server, carrying the advice it offered.
 *
 * The server explains what to do about a problem as well as what the problem
 * was, and discarding that advice on the way through the client would leave the
 * interface to invent its own worse version.
 */
export class ApiError extends Error {
  readonly status: number
  readonly advice: string[]

  constructor(status: number, message: string, advice: string[]) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.advice = advice
  }
}

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

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`/api/v1${path}`, {
    ...init,
    headers: { Accept: 'application/json', 'Content-Type': 'application/json', ...init?.headers },
  })
  if (response.ok) {
    // A deletion succeeds with no body, and asking for JSON that was never sent
    // would turn it into a failure.
    return response.status === 204 ? (undefined as T) : ((await response.json()) as T)
  }
  // A refusal is expected to be JSON, but a proxy or a crash can produce
  // something else, and the status is worth reporting either way.
  const failure = await response.json().catch(() => null)
  throw new ApiError(
    response.status,
    failure?.message ?? `The request failed with status ${response.status}.`,
    failure?.advice ?? [],
  )
}

export const api = {
  designs: () => request<DesignSummary[]>('/designs'),

  create: (id: string, name: string, summary: string) =>
    request<Snapshot>('/designs', {
      method: 'POST',
      body: JSON.stringify({ id, name, summary }),
    }),

  design: (design: string) => request<Snapshot>(`/designs/${encodeURIComponent(design)}`),

  /** Deletes a design and everything stored under it. */
  remove: (design: string) =>
    request<void>(`/designs/${encodeURIComponent(design)}`, { method: 'DELETE' }),

  catalogue: (design: string) =>
    request<Catalogue>(`/designs/${encodeURIComponent(design)}/catalogue`),

  /**
   * Applies edits in order, stopping at the first that will not apply.
   *
   * Earlier edits in a rejected batch stand, because each names one entity and
   * is complete on its own. The returned count says how many landed.
   */
  mutate: (design: string, mutations: Mutation[]) =>
    request<Applied>(`/designs/${encodeURIComponent(design)}/mutations`, {
      method: 'POST',
      body: JSON.stringify({ mutations }),
    }),

  analysis: (design: string, controls: SolveControls = {}) =>
    request<Analysis>(`/designs/${encodeURIComponent(design)}/analysis${query(controls)}`),

  /**
   * Evaluates one expression the way the solver would, for a preview.
   *
   * `entry` names the shared quantity being edited, because a quantity sees only
   * the ones declared ahead of it and a preview that ignored that would show a
   * figure the solver is going to refuse.
   */
  preview: (design: string, expression: string, entry: string | null = null) =>
    request<Quantity>(`/designs/${encodeURIComponent(design)}/preview`, {
      method: 'POST',
      body: JSON.stringify({ expression, entry }),
    }),

  comparison: (design: string, intervention: string, controls: SolveControls = {}) =>
    request<Comparison>(
      `/designs/${encodeURIComponent(design)}/comparisons/${encodeURIComponent(intervention)}${query(
        controls,
      )}`,
    ),
}
