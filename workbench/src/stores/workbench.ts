import { defineStore } from 'pinia'
import { ref } from 'vue'

/** Which part of a design is on screen. */
export type View = 'design' | 'bottlenecks' | 'compare'

/**
 * What the workbench is looking at.
 *
 * Only the selections a URL should be able to restore live here. Everything
 * fetched from the server — the design, the catalogue, an analysis — belongs to
 * the query layer, so that state which can go stale is never held somewhere
 * nothing knows to refresh it.
 */
export const useWorkbenchStore = defineStore('workbench', () => {
  const design = ref<string | null>(null)
  const view = ref<View>('design')
  const intervention = ref<string | null>(null)
  const samples = ref(1000)
  const horizon = ref(1)

  function open(id: string | null) {
    if (design.value === id) return
    design.value = id
    // A proposal belongs to the design that declared it, so carrying a selection
    // across would ask the server about something that does not exist there.
    intervention.value = null
  }

  return { design, view, intervention, samples, horizon, open }
})
