import { computed, type Ref } from 'vue'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from '../api/client'
import type {
  AppendObservationInput,
  CorrectObservationInput,
  CreateEdgeInput,
  CreateNodeInput,
  GraphNode,
  GraphEdge,
  Project,
  ProjectArchive,
  SetStateEstimateInput,
  SetInterventionEstimateInput,
  Estimate,
  Evidence,
  EvidenceInput,
  UpdateNodeInput,
  UpdateEdgeInput,
} from '../api/types'

export function useProjects() {
  return useQuery({ queryKey: ['projects'], queryFn: api.projects })
}

export function useProject(projectId: Ref<string | null>) {
  const enabled = computed(() => Boolean(projectId.value))
  return useQuery({
    queryKey: computed(() => ['project', projectId.value]),
    queryFn: () => api.project(projectId.value!),
    enabled,
  })
}

export function useGraph(projectId: Ref<string | null>) {
  const enabled = computed(() => Boolean(projectId.value))
  const nodes = useQuery({
    queryKey: computed(() => ['nodes', projectId.value]),
    queryFn: () => api.nodes(projectId.value!),
    enabled,
  })
  const edges = useQuery({
    queryKey: computed(() => ['edges', projectId.value]),
    queryFn: () => api.edges(projectId.value!),
    enabled,
  })
  return { nodes, edges }
}

export function useCreateProject() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: api.createProject,
    onSuccess: (project) => {
      queryClient.setQueryData<Project[]>(['projects'], (projects = []) => [
        ...projects,
        project,
      ])
    },
  })
}

export function useImportProject() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ archive, replace }: { archive: ProjectArchive; replace: boolean }) =>
      api.importProject(archive, replace),
    onSuccess: async () => {
      await queryClient.invalidateQueries()
    },
  })
}

export function useCreateNode(project: Ref<Project | undefined>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: CreateNodeInput) => api.createNode(project.value!, input),
    onSuccess: async () => {
      const id = project.value!.id
      await Promise.all([
        queryClient.refetchQueries({ queryKey: ['project', id] }),
        queryClient.refetchQueries({ queryKey: ['nodes', id] }),
      ])
    },
  })
}

export function useCreateEdge(project: Ref<Project | undefined>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: CreateEdgeInput) => api.createEdge(project.value!, input),
    onSuccess: async () => {
      const id = project.value!.id
      await Promise.all([
        queryClient.refetchQueries({ queryKey: ['project', id] }),
        queryClient.refetchQueries({ queryKey: ['edges', id] }),
      ])
    },
  })
}

async function refetchNodeData(queryClient: ReturnType<typeof useQueryClient>, project: Project) {
  await Promise.all([
    queryClient.refetchQueries({ queryKey: ['project', project.id] }),
    queryClient.refetchQueries({ queryKey: ['nodes', project.id] }),
  ])
}

export function useUpdateNode(project: Ref<Project | undefined>, node: Ref<GraphNode | null>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: UpdateNodeInput) => api.updateNode(project.value!, node.value!, input),
    onSuccess: async () => refetchNodeData(queryClient, project.value!),
  })
}

export function useSetStateEstimate(
  project: Ref<Project | undefined>,
  node: Ref<GraphNode | null>,
) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: SetStateEstimateInput) =>
      api.setStateEstimate(project.value!, node.value!, input),
    onSuccess: async () => refetchNodeData(queryClient, project.value!),
  })
}

async function refetchEdgeData(queryClient: ReturnType<typeof useQueryClient>, project: Project) {
  await Promise.all([
    queryClient.refetchQueries({ queryKey: ['project', project.id] }),
    queryClient.refetchQueries({ queryKey: ['edges', project.id] }),
  ])
}

export function useUpdateEdge(project: Ref<Project | undefined>, edge: Ref<GraphEdge | null>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: UpdateEdgeInput) => api.updateEdge(project.value!, edge.value!, input),
    onSuccess: async () => refetchEdgeData(queryClient, project.value!),
  })
}

export function useDeleteEdge(project: Ref<Project | undefined>, edge: Ref<GraphEdge | null>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => api.deleteEdge(project.value!, edge.value!),
    onSuccess: async () => refetchEdgeData(queryClient, project.value!),
  })
}

export function useDeleteNode(project: Ref<Project | undefined>, node: Ref<GraphNode | null>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => api.deleteNode(project.value!, node.value!),
    onSuccess: async () => refetchNodeData(queryClient, project.value!),
  })
}

export function useAppendObservation(
  project: Ref<Project | undefined>,
  edge: Ref<GraphEdge | null>,
) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: AppendObservationInput) =>
      api.appendObservation(project.value!, edge.value!, input),
    onSuccess: async () => refetchEdgeData(queryClient, project.value!),
  })
}

export function useCorrectObservation(
  project: Ref<Project | undefined>,
  edge: Ref<GraphEdge | null>,
) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: CorrectObservationInput) =>
      api.correctObservation(project.value!, edge.value!, input),
    onSuccess: async () => refetchEdgeData(queryClient, project.value!),
  })
}

export function useSetInterventionEstimate(
  project: Ref<Project | undefined>,
  node: Ref<GraphNode | null>,
) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: SetInterventionEstimateInput) =>
      api.setInterventionEstimate(project.value!, node.value!, input),
    onSuccess: async () => refetchNodeData(queryClient, project.value!),
  })
}

export function useRemoveInterventionEstimate(
  project: Ref<Project | undefined>,
  node: Ref<GraphNode | null>,
) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (estimate: Estimate) =>
      api.removeInterventionEstimate(project.value!, node.value!, estimate),
    onSuccess: async () => refetchNodeData(queryClient, project.value!),
  })
}

export function useCreateEvidence(project: Ref<Project | undefined>, node: Ref<GraphNode | null>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: EvidenceInput) => api.createEvidence(project.value!, node.value!, input),
    onSuccess: async () => refetchNodeData(queryClient, project.value!),
  })
}

export function useUpdateEvidence(
  project: Ref<Project | undefined>,
  node: Ref<GraphNode | null>,
  evidence: Ref<Evidence | null>,
) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: EvidenceInput) =>
      api.updateEvidence(project.value!, node.value!, evidence.value!, input),
    onSuccess: async () => refetchNodeData(queryClient, project.value!),
  })
}

export function useDeleteEvidence(
  project: Ref<Project | undefined>,
  node: Ref<GraphNode | null>,
  evidence: Ref<Evidence | null>,
) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => api.deleteEvidence(project.value!, node.value!, evidence.value!),
    onSuccess: async () => refetchNodeData(queryClient, project.value!),
  })
}