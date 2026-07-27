/**
 * Reading a solver refusal.
 *
 * The server explains what went wrong in a sentence meant for a person, and that
 * sentence names the component it was evaluating. Pulling the name out lets the
 * interface put the complaint next to the thing that caused it rather than in a
 * banner at the top of the page, which is the difference between "this design is
 * broken" and "this field is empty".
 *
 * Deliberately a light touch. If the wording changes the name is simply not
 * found, and the message is still shown in full — nothing depends on the parse
 * succeeding.
 */
export interface Problem {
  /** The component the solver was working on, where the message names one. */
  component: string | null
  /** The message, always shown whether or not a component was identified. */
  message: string
  advice: string[]
}

const NAMES_A_COMPONENT = /component '([^']+)'/

export function readProblem(error: unknown): Problem | null {
  if (!error) return null
  const message = error instanceof Error ? error.message : String(error)
  const advice = (error as { advice?: string[] }).advice ?? []
  return { component: NAMES_A_COMPONENT.exec(message)?.[1] ?? null, message, advice }
}
