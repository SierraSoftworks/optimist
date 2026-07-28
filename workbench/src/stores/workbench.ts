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
  /**
   * Whether to walk the design through time rather than solve for balance.
   *
   * Off by default because balance is what somebody editing a design wants: it
   * answers immediately and says where the design comes to rest. Turning this on
   * makes each queue fill and drain at a finite rate, which is the only way to
   * see how long an incident outlasts its cause — and costs enough that it is a
   * deliberate act rather than something to leave running.
   */
  const transient = ref(false)
  /**
   * Seconds each step covers.
   *
   * Only meaningful when walking through time, where integrating faithfully
   * needs a step short against the time a queue takes to drain. A second is far
   * too long for a service answering in milliseconds, which is why turning
   * transient on shortens it.
   */
  const step = ref(1)

  return { samples, horizon, seed, transient, step }
})
