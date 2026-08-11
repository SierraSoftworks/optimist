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

/**
 * Reads the advice out of a refusal, however the sender phrased it.
 *
 * One suggestion is sent as a string and several as a list, because most
 * refusals have one thing to say and only some have more.
 */
export function adviceLines(advice: unknown): string[] {
  if (typeof advice === 'string') return [advice]
  if (Array.isArray(advice)) return advice.filter((line): line is string => typeof line === 'string')
  return []
}
