import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, onScopeDispose, ref, toValue, watch, type MaybeRefOrGetter } from 'vue'

import { api, type SolveControls } from '../api/client'
import { watchDesign, type FeedStatus } from '../api/feed'
import type { Mutation, Snapshot } from '../api/types'
import { applyMutation } from '../domain/applyMutation'
import { useSolvingStore } from '../stores/solving'

/** The designs this server holds. */
export function useDesigns() {
  return useQuery({ queryKey: ['designs'], queryFn: api.designs })
}

/** Everything a design may be built from. */
export function useCatalogue(design: MaybeRefOrGetter<string | null>) {
  return useQuery({
    queryKey: computed(() => ['catalogue', toValue(design)]),
    queryFn: () => api.catalogue(toValue(design)!),
    enabled: computed(() => toValue(design) !== null),
    // A catalogue changes when a file on disk does, which is not something the
    // feed reports. Refetching it on every focus is cheap and avoids showing a
    // component type that has been edited away.
    staleTime: 30_000,
  })
}

/**
 * A design, kept current from its change feed.
 *
 * Edits arrive as the same messages the server applied and are replayed onto the
 * local copy, rather than triggering a refetch. Refetching would replace the
 * whole design, discarding whatever is half-typed in a field somewhere on the
 * page; replaying touches only the entity the edit names.
 */
export function useDesign(design: MaybeRefOrGetter<string | null>) {
  const client = useQueryClient()
  const solving = useSolvingStore()
  const status = ref<FeedStatus>('closed')
  const key = computed(() => ['design', toValue(design)])

  const query = useQuery({
    queryKey: key,
    queryFn: () => api.design(toValue(design)!),
    enabled: computed(() => toValue(design) !== null),
  })

  let stop: (() => void) | null = null
  watch(
    () => toValue(design),
    (id) => {
      stop?.()
      stop = null
      if (!id) return
      stop = watchDesign(id, {
        onStatusChange: (next) => {
          status.value = next
          // A dropped socket stops reporting solves without stopping them, and a
          // bar left turning over a connection that is gone is a lie. The list
          // arrives again with the snapshot when the socket comes back.
          if (next !== 'open') solving.forget(id)
        },
        onMessage: (message) => {
          const cacheKey = ['design', id]
          if (message.type === 'snapshot') {
            const { type: _type, ...snapshot } = message
            client.setQueryData(cacheKey, snapshot as Snapshot)
            return
          }
          if (message.type === 'active') {
            solving.replace(id, message.solves)
            return
          }
          if (message.type === 'solving') {
            solving.update(id, message.solve)
            return
          }
          if (message.type === 'solved') {
            solving.finish(id, message.solve)
            return
          }
          if (message.type === 'lagged') {
            // Changes were dropped, so the local copy cannot be repaired by
            // replay. This is the one case where refetching is the right answer.
            void client.invalidateQueries({ queryKey: cacheKey })
            return
          }
          client.setQueryData(cacheKey, (current: Snapshot | undefined) =>
            current && message.sequence > current.sequence
              ? {
                  ...current,
                  model: applyMutation(current.model, message.mutation),
                  sequence: message.sequence,
                }
              : current,
          )
        },
      })
    },
    { immediate: true },
  )
  onScopeDispose(() => stop?.())

  return { ...query, feedStatus: status }
}

/**
 * Applies edits to a design.
 *
 * The result is not written into the cache here. The same edit comes back over
 * the feed, and letting that one path update the design means an edit made in
 * this tab and one made in another are handled by identical code rather than by
 * two implementations that can disagree.
 */
export function useEditDesign(design: MaybeRefOrGetter<string | null>) {
  return useMutation({
    mutationFn: (mutations: Mutation[]) => api.mutate(toValue(design)!, mutations),
  })
}

/** A solved design and what constrains it. */
export function useAnalysis(
  design: MaybeRefOrGetter<string | null>,
  controls: MaybeRefOrGetter<SolveControls>,
  sequence: MaybeRefOrGetter<number | undefined>,
) {
  return useQuery({
    // The sequence is part of the key, so an edit landing on the feed asks for a
    // fresh answer without anything having to remember to invalidate one.
    queryKey: computed(() => ['analysis', toValue(design), toValue(sequence), toValue(controls)]),
    queryFn: () => api.analysis(toValue(design)!, toValue(controls)),
    enabled: computed(() => toValue(design) !== null),
    placeholderData: (previous) => previous,
  })
}

/** A proposal weighed against the design it would replace. */
export function useComparison(
  design: MaybeRefOrGetter<string | null>,
  intervention: MaybeRefOrGetter<string | null>,
  controls: MaybeRefOrGetter<SolveControls>,
  sequence: MaybeRefOrGetter<number | undefined>,
) {
  return useQuery({
    queryKey: computed(() => [
      'comparison',
      toValue(design),
      toValue(intervention),
      toValue(sequence),
      toValue(controls),
    ]),
    queryFn: () => api.comparison(toValue(design)!, toValue(intervention)!, toValue(controls)),
    enabled: computed(() => toValue(design) !== null && toValue(intervention) !== null),
    placeholderData: (previous) => previous,
  })
}
