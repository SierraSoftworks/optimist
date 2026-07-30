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

  /**
   * Turns walking through time on or off, moving the step and horizon with it.
   *
   * The three are one decision. A step of a second cannot integrate a service
   * answering in milliseconds, and a horizon of twenty short steps covers no
   * time at all, so either without the others is misleading — which is why this
   * is an action rather than three fields a caller has to remember to set.
   */
  function walkThroughTime(wanted: boolean) {
    transient.value = wanted
    step.value = wanted ? 0.5 : 1
    horizon.value = wanted ? 120 : 20
  }

  /**
   * Which quantities each design is being watched through, in reading order.
   *
   * Held here rather than in the view that draws them because a reader assembles
   * this list a click at a time and then goes off to change the design that
   * produced it. Rebuilding it on the way back would make the two views feel
   * like two applications. Kept per design, since a selection names quantities
   * that only exist in the design it was made against.
   *
   * Left out of the URL deliberately: it is a working set rather than something
   * somebody sends to a colleague, and it is long enough to make a link
   * unreadable.
   */
  const watching = ref<Record<string, string[]>>({})

  function watched(design: string): string[] {
    return watching.value[design] ?? []
  }

  function watch(design: string, signals: string[]) {
    watching.value[design] = signals
  }

  return { samples, horizon, seed, transient, step, walkThroughTime, watched, watch }
})
