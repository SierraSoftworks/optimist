import { defineStore } from 'pinia'
import { ref } from 'vue'

import type { RunningSolve, SolveTarget } from '../api/types'

/**
 * What the server is solving right now, for every design being watched.
 *
 * Fed entirely from the change feed, which reports solves to everyone watching a
 * design rather than to whoever started one. That is what lets the rail show a
 * colleague's solve turning over, and what lets a reloaded page pick the picture
 * back up: the socket opens with the list of what is already running.
 *
 * Nothing here is matched against what this client asked for. The server answers
 * one question per design position at a time, so a solve of a variant is *the*
 * solve of that variant, whoever set it going.
 */
export const useSolvingStore = defineStore('solving', () => {
  const running = ref<Record<string, Record<string, RunningSolve>>>({})

  /** A solve is named by what it is solving, which is all a reader needs. */
  const name = (solve: SolveTarget) => `${solve.kind}:${solve.variant ?? ''}`

  function replace(design: string, solves: RunningSolve[]) {
    running.value = {
      ...running.value,
      [design]: Object.fromEntries(solves.map((solve) => [name(solve), solve])),
    }
  }

  function update(design: string, solve: RunningSolve) {
    const held = running.value[design]?.[name(solve)]
    // Two people asking the same question with different sample counts are two
    // solves under one name. Showing the one that has got furthest keeps the
    // indicator from walking backwards as their frames interleave.
    if (held && held.sequence === solve.sequence && held.fraction > solve.fraction) return
    running.value = {
      ...running.value,
      [design]: { ...running.value[design], [name(solve)]: solve },
    }
  }

  function finish(design: string, solve: SolveTarget) {
    const held = running.value[design]
    if (!held || !(name(solve) in held)) return
    const { [name(solve)]: _done, ...rest } = held
    running.value = { ...running.value, [design]: rest }
  }

  function forget(design: string) {
    const { [design]: _gone, ...rest } = running.value
    running.value = rest
  }

  /** Everything running for a design, for the rail to draw against its rows. */
  function solves(design: string | null): Record<string, RunningSolve> {
    return (design && running.value[design]) || {}
  }

  /**
   * The solve of one variant, whichever question it is answering.
   *
   * A variant being weighed against the design it would replace is still that
   * variant being solved, so far as the row showing it is concerned.
   */
  function variant(design: string | null, id: string | null): RunningSolve | null {
    const held = solves(design)
    return held[`analysis:${id ?? ''}`] ?? held[`comparison:${id ?? ''}`] ?? null
  }

  return { running, replace, update, finish, forget, solves, variant }
})
