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
  Scenario,
  ScenarioDraft,
  SetStateEstimateInput,
  SetInterventionEstimateInput,
  Estimate,
  EdgeEstimateSlot,
  Evidence,
  EvidenceInput,
  SetEdgeEstimateInput,
  SetMeasurementCalibrationInput,
  SetEffectProfileInput,
  UpdateNodeInput,
  UpdateEdgeInput,
} from '../api/types'

export function useProjects() {
  return useQuery({ queryKey: ['projects'], queryFn: api.projects })
}

export function useServerHealth() {
  return useQuery({
    queryKey: ['health'],
    queryFn: api.health,
    refetchInterval: 1_000,
  })
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

export function useStructuralAnalysis(
  projectId: Ref<string | null>,
  projectRevision: Ref<number | undefined>,
  enabled: Ref<boolean>,
) {
  return useQuery({
    queryKey: computed(() => [
      'analysis', 'structure', projectId.value, projectRevision.value,
    ]),
    queryFn: () => api.structuralAnalysis(projectId.value!),
    enabled: computed(() => Boolean(projectId.value) && enabled.value),
  })
}

export function useImpedimentAnalysis(
  projectId: Ref<string | null>,
  projectRevision: Ref<number | undefined>,
  enabled: Ref<boolean>,
) {
  return useQuery({
    queryKey: computed(() => [
      'analysis', 'impediments', projectId.value, projectRevision.value,
    ]),
    queryFn: () => api.impedimentAnalysis(projectId.value!),
    enabled: computed(() => Boolean(projectId.value) && enabled.value),
  })
}

export function useScenarios(
  projectId: Ref<string | null>,
  enabled: Ref<boolean>,
) {
  return useQuery({
    queryKey: computed(() => ['scenarios', projectId.value]),
    queryFn: () => api.scenarios(projectId.value!),
    enabled: computed(() => Boolean(projectId.value) && enabled.value),
  })
}

export function useScenarioAnalysis(
  projectId: Ref<string | null>,
  scenarioId: Ref<string | null>,
  scenarioRevision: Ref<number | undefined>,
  enabled: Ref<boolean>,
) {
  return useQuery({
    queryKey: computed(() => [
      'analysis', 'scenario', projectId.value, scenarioId.value, scenarioRevision.value,
    ]),
    queryFn: () => api.scenarioAnalysis(projectId.value!, scenarioId.value!),
    enabled: computed(() =>
      Boolean(projectId.value) && Boolean(scenarioId.value) && enabled.value,
    ),
  })
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

export function useCreateScenario(project: Ref<Project | undefined>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (scenario: ScenarioDraft) => api.createScenario(project.value!, scenario),
    onSuccess: (scenario) => {
      const current = project.value!
      advanceProject(queryClient, current)
      queryClient.setQueryData<Scenario[]>(['scenarios', current.id], (scenarios = []) => [...scenarios, scenario])
      invalidateAnalysis(queryClient, current.id)
    },
  })
}

export function useUpdateScenario(
  project: Ref<Project | undefined>,
  scenario: Ref<Scenario | null>,
) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (draft: ScenarioDraft) =>
      api.updateScenario(project.value!, scenario.value!, draft),
    onSuccess: (updated) => {
      const current = project.value!
      advanceProject(queryClient, current)
      queryClient.setQueryData<Scenario[]>(['scenarios', current.id], (scenarios = []) =>
        scenarios.map((scenario) => scenario.id === updated.id ? updated : scenario),
      )
      invalidateAnalysis(queryClient, current.id)
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
    onSuccess: (node) => {
      const id = project.value!.id
      advanceProject(queryClient, project.value!)
      queryClient.setQueryData<GraphNode[]>(['nodes', id], (nodes = []) => [...nodes, node])
      invalidateAnalysis(queryClient, id)
    },
  })
}

export function useCreateEdge(project: Ref<Project | undefined>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: CreateEdgeInput) => api.createEdge(project.value!, input),
    onSuccess: (edge) => {
      const id = project.value!.id
      advanceProject(queryClient, project.value!)
      queryClient.setQueryData<GraphEdge[]>(['edges', id], (edges = []) => [...edges, edge])
      invalidateAnalysis(queryClient, id)
    },
  })
}

function advanceProject(queryClient: ReturnType<typeof useQueryClient>, project: Project) {
  queryClient.setQueryData<Project>(['project', project.id], (current) => ({
    ...(current ?? project),
    revision: Math.max(current?.revision ?? project.revision, project.revision) + 1,
  }))
}

function invalidateAnalysis(queryClient: ReturnType<typeof useQueryClient>, project: string) {
  void queryClient.invalidateQueries({ queryKey: ['analysis'], predicate: (query) => query.queryKey.includes(project) })
}

function refreshNodes(queryClient: ReturnType<typeof useQueryClient>, project: Project) {
  advanceProject(queryClient, project)
  void queryClient.invalidateQueries({ queryKey: ['nodes', project.id] })
  invalidateAnalysis(queryClient, project.id)
}

function refreshEdges(queryClient: ReturnType<typeof useQueryClient>, project: Project) {
  advanceProject(queryClient, project)
  void queryClient.invalidateQueries({ queryKey: ['edges', project.id] })
  invalidateAnalysis(queryClient, project.id)
}

export function useUpdateNode(project: Ref<Project | undefined>, node: Ref<GraphNode | null>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: UpdateNodeInput) => api.updateNode(project.value!, node.value!, input),
    onSuccess: (updated) => {
      const current = project.value!
      advanceProject(queryClient, current)
      queryClient.setQueryData<GraphNode[]>(['nodes', current.id], (nodes = []) =>
        nodes.map((node) => node.id === updated.id ? updated : node),
      )
      invalidateAnalysis(queryClient, current.id)
    },
  })
}

export function useSetNodeQuantityState(
  project: Ref<Project | undefined>,
  node: Ref<GraphNode | null>,
) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: import('../api/types').SetNodeQuantityStateInput) =>
      api.setNodeQuantityState(project.value!, node.value!, input),
    onSuccess: (updated) => {
      const current = project.value!
      advanceProject(queryClient, current)
      queryClient.setQueryData<GraphNode[]>(['nodes', current.id], (nodes = []) =>
        nodes.map((node) => node.id === updated.id ? updated : node),
      )
      invalidateAnalysis(queryClient, current.id)
    },
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
    onSuccess: () => refreshNodes(queryClient, project.value!),
  })
}

export function useUpdateEdge(project: Ref<Project | undefined>, edge: Ref<GraphEdge | null>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: UpdateEdgeInput) => api.updateEdge(project.value!, edge.value!, input),
    onSuccess: (updated) => {
      const current = project.value!
      advanceProject(queryClient, current)
      queryClient.setQueryData<GraphEdge[]>(['edges', current.id], (edges = []) =>
        edges.map((edge) => edge.source === updated.source && edge.destination === updated.destination && edge.payload.kind === updated.payload.kind ? updated : edge),
      )
      invalidateAnalysis(queryClient, current.id)
    },
  })
}

export function useSetMeasurementCalibration(
  project: Ref<Project | undefined>,
  edge: Ref<GraphEdge | null>,
) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: SetMeasurementCalibrationInput) =>
      api.setMeasurementCalibration(project.value!, edge.value!, input),
    onSuccess: (updated) => {
      const current = project.value!
      advanceProject(queryClient, current)
      queryClient.setQueryData<GraphEdge[]>(['edges', current.id], (edges = []) =>
        edges.map((edge) => edge.source === updated.source && edge.destination === updated.destination && edge.payload.kind === updated.payload.kind ? updated : edge),
      )
      invalidateAnalysis(queryClient, current.id)
    },
  })
}

export function useSetEffectProfile(
  project: Ref<Project | undefined>,
  edge: Ref<GraphEdge | null>,
) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: SetEffectProfileInput) =>
      api.setEffectProfile(project.value!, edge.value!, input),
    onSuccess: (updated) => {
      const current = project.value!
      advanceProject(queryClient, current)
      queryClient.setQueryData<GraphEdge[]>(['edges', current.id], (edges = []) =>
        edges.map((edge) => edge.source === updated.source && edge.destination === updated.destination && edge.payload.kind === updated.payload.kind ? updated : edge),
      )
      invalidateAnalysis(queryClient, current.id)
    },
  })
}

export function useDeleteEdge(project: Ref<Project | undefined>, edge: Ref<GraphEdge | null>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => api.deleteEdge(project.value!, edge.value!),
    onSuccess: (deleted) => {
      const current = project.value!
      advanceProject(queryClient, current)
      queryClient.setQueryData<GraphEdge[]>(['edges', current.id], (edges = []) =>
        edges.filter((edge) => !(edge.source === deleted.source && edge.destination === deleted.destination && edge.payload.kind === deleted.payload.kind)),
      )
      invalidateAnalysis(queryClient, current.id)
    },
  })
}

export function useDeleteNode(project: Ref<Project | undefined>, node: Ref<GraphNode | null>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => api.deleteNode(project.value!, node.value!),
    onSuccess: (deleted) => {
      const current = project.value!
      advanceProject(queryClient, current)
      queryClient.setQueryData<GraphNode[]>(['nodes', current.id], (nodes = []) => nodes.filter((node) => node.id !== deleted.id))
      invalidateAnalysis(queryClient, current.id)
    },
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
    onSuccess: () => refreshEdges(queryClient, project.value!),
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
    onSuccess: () => refreshEdges(queryClient, project.value!),
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
    onSuccess: () => refreshNodes(queryClient, project.value!),
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
    onSuccess: () => refreshNodes(queryClient, project.value!),
  })
}

export function useCreateEvidence(project: Ref<Project | undefined>, node: Ref<GraphNode | null>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: EvidenceInput) => api.createEvidence(project.value!, node.value!, input),
    onSuccess: () => refreshNodes(queryClient, project.value!),
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
    onSuccess: () => refreshNodes(queryClient, project.value!),
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
    onSuccess: () => refreshNodes(queryClient, project.value!),
  })
}

export function useSetEdgeEstimate(
  project: Ref<Project | undefined>,
  edge: Ref<GraphEdge | null>,
) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (input: SetEdgeEstimateInput) => {
      const result = await api.setEdgeEstimate(project.value!, edge.value!, input)
      return setEdgeEstimate(edge.value!, input.slot.kind, {
        id: result.address.estimate,
        revision: result.revision,
        distribution: result.distribution,
        source: result.source,
        provenance: result.provenance,
      })
    },
    onSuccess: (updated) => updateCachedEdge(queryClient, project.value!, updated),
  })
}

export function useRemoveEdgeEstimate(
  project: Ref<Project | undefined>,
  edge: Ref<GraphEdge | null>,
) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (estimate: Estimate) => {
      await api.removeEdgeEstimate(project.value!, edge.value!, estimate)
      return removeCachedEdgeEstimate(edge.value!, estimate.id)
    },
    onSuccess: (updated) => updateCachedEdge(queryClient, project.value!, updated),
  })
}

function updateCachedEdge(
  queryClient: ReturnType<typeof useQueryClient>,
  project: Project,
  updated: GraphEdge,
) {
  advanceProject(queryClient, project)
  queryClient.setQueryData<GraphEdge[]>(['edges', project.id], (edges = []) =>
    edges.map((edge) => sameEdge(edge, updated) ? updated : edge),
  )
  invalidateAnalysis(queryClient, project.id)
}

function sameEdge(left: GraphEdge, right: GraphEdge) {
  return left.source === right.source &&
    left.destination === right.destination &&
    left.payload.kind === right.payload.kind
}

function setEdgeEstimate(edge: GraphEdge, slot: EdgeEstimateSlot['kind'], estimate: Estimate) {
  const updated = cloneEdge(edge)
  updated.revision += 1
  if (updated.payload.kind === 'contributes' || updated.payload.kind === 'changes') {
    if (slot === 'response') updated.payload.properties.response.destination_change = estimate
    if (slot === 'lag') updated.payload.properties.lag = estimate
  }
  if (slot === 'degree' && updated.payload.kind === 'blocks') updated.payload.properties.degree = estimate
  return updated
}

function removeCachedEdgeEstimate(edge: GraphEdge, estimateId: string) {
  const updated = cloneEdge(edge)
  updated.revision += 1
  if ((updated.payload.kind === 'contributes' || updated.payload.kind === 'changes') && updated.payload.properties.lag?.id === estimateId) {
    updated.payload.properties.lag = null
  }
  return updated
}

function cloneEdge(edge: GraphEdge): GraphEdge {
  return JSON.parse(JSON.stringify(edge)) as GraphEdge
}