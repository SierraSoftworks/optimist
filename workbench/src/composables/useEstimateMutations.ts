import type { Ref } from 'vue'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { api } from '../api/client'
import type {
  Estimate,
  GraphNode,
  Project,
  SetInterventionEstimateInput,
  SetStateEstimateInput,
} from '../api/types'
import {
  setInterventionEstimate as cacheInterventionEstimate,
  setStateEstimate as cacheStateEstimate,
} from '../domain/estimateCache'

function returnedEstimate(result: Awaited<ReturnType<typeof api.setStateEstimate>>): Estimate {
  return {
    id: result.address.estimate,
    revision: result.revision,
    distribution: result.distribution,
    quantity: result.quantity,
    source: result.source,
    provenance: result.provenance,
    uncertainty: result.uncertainty,
  }
}

function updateCachedNode(
  queryClient: ReturnType<typeof useQueryClient>,
  project: Project,
  updated: GraphNode,
) {
  queryClient.setQueryData<Project>(['project', project.id], (current) => ({
    ...(current ?? project),
    revision: Math.max(current?.revision ?? project.revision, project.revision) + 1,
  }))
  queryClient.setQueryData<GraphNode[]>(['nodes', project.id], (nodes = []) =>
    nodes.map((node) => node.id === updated.id ? updated : node),
  )
  void queryClient.invalidateQueries({
    queryKey: ['analysis'],
    predicate: (query) => query.queryKey.includes(project.id),
  })
}

export function useSetStateEstimate(
  project: Ref<Project | undefined>,
  node: Ref<GraphNode | null>,
) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (input: SetStateEstimateInput) => {
      const result = await api.setStateEstimate(project.value!, node.value!, input)
      return cacheStateEstimate(node.value!, input.slot, returnedEstimate(result))
    },
    onSuccess: (updated) => updateCachedNode(queryClient, project.value!, updated),
  })
}

export function useSetInterventionEstimate(
  project: Ref<Project | undefined>,
  node: Ref<GraphNode | null>,
) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (input: SetInterventionEstimateInput) => {
      const result = await api.setInterventionEstimate(project.value!, node.value!, input)
      return cacheInterventionEstimate(node.value!, input.slot, returnedEstimate(result))
    },
    onSuccess: (updated) => updateCachedNode(queryClient, project.value!, updated),
  })
}
