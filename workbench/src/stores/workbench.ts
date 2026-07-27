import { defineStore } from 'pinia'
import { ref } from 'vue'

/**
 * How hard the solver is asked to work.
 *
 * Which design is open, which mode it is in, and what is selected all live in
 * the URL, because those are the things somebody sends to a colleague. What is
 * left here describes how a screen was produced rather than what is on it, so it
 * is deliberately not addressable: a link should not carry a sample count that
 * silently makes the recipient's machine work harder than the sender's did.
 */
export const useWorkbenchStore = defineStore('workbench', () => {
  const samples = ref(1000)
  // Long enough that a chart over time has something to show, and short enough
  // that a design still solves while somebody is typing into it.
  const horizon = ref(20)
  const seed = ref(0)

  return { samples, horizon, seed }
})
