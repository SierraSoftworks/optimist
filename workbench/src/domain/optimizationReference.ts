import type { ScenarioAnalysis } from '../api/types'

type Candidate = ScenarioAnalysis['candidates'][number]

/**
 * The run a candidate should be read against.
 *
 * A candidate that requires other interventions is never compared against doing
 * nothing, because its prerequisites run whether or not it does. Judging load
 * shedding against a quiet system would credit it with the entire load surge it
 * was evaluated under; the honest comparison is the surge alone.
 *
 * That run is often already in the analysis, because a scenario that offers an
 * intervention usually offers its prerequisites as candidates too. This finds it
 * by execution plan rather than by name: the reference for a candidate is
 * whichever other candidate executes exactly the set this one requires.
 */
export function referenceCandidate(
  candidate: Candidate,
  candidates: Candidate[],
): Candidate | null {
  if (!candidate.prerequisites.length) return null
  const required = new Set(candidate.prerequisites)
  return (
    candidates.find((other) => {
      if (other.intervention === candidate.intervention) return false
      const executes = [other.intervention, ...other.prerequisites]
      return executes.length === required.size && executes.every((id) => required.has(id))
    }) ?? null
  )
}

/**
 * Values of the run a candidate deviates from, period by period.
 *
 * With no reference run the outcome holds at its resting level, which is what
 * "without this intervention" means for a candidate that requires nothing. A
 * candidate whose prerequisites are not themselves candidates has no run to
 * compare against, and gets the same resting level with the shortfall named in
 * the caption rather than a silently wrong curve.
 */
export function referenceStates(
  candidate: Candidate,
  outcome: string,
  reference: Candidate | null,
  periods: number,
): Array<number | null> {
  const objective = reference?.objectives.find((entry) => entry.outcome === outcome)
  if (objective) {
    return objective.trajectory.map((point) => point.state.mean)
  }
  const resting = candidate.objectives.find((entry) => entry.outcome === outcome)?.baseline.mean
  return Array.from({ length: periods }, () => resting ?? null)
}
