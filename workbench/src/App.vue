<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import {
  Activity,
  AlertTriangle,
  ChevronDown,
  CircleDot,
  Gauge,
  Goal,
  Download,
  Upload,
  Link,
  Network,
  Plus,
  RefreshCw,
  Search,
  SquareTerminal,
  Wrench,
} from '@lucide/vue'
import GraphCanvas from './components/GraphCanvas.vue'
import NodeInspector from './components/NodeInspector.vue'
import CreateProjectDialog from './components/CreateProjectDialog.vue'
import CreateNodeDialog from './components/CreateNodeDialog.vue'
import CreateEdgeDialog from './components/CreateEdgeDialog.vue'
import DeleteSelectionDialog from './components/DeleteSelectionDialog.vue'
import ImportProjectDialog from './components/ImportProjectDialog.vue'
import EditNodeDialog from './components/EditNodeDialog.vue'
import EditStateEstimateDialog from './components/EditStateEstimateDialog.vue'
import EditEdgeDialog from './components/EditEdgeDialog.vue'
import AddObservationDialog from './components/AddObservationDialog.vue'
import CorrectObservationDialog from './components/CorrectObservationDialog.vue'
import EditInterventionEstimateDialog from './components/EditInterventionEstimateDialog.vue'
import EditEvidenceDialog from './components/EditEvidenceDialog.vue'
import EditEdgeEstimateDialog from './components/EditEdgeEstimateDialog.vue'
import GraphNavigator from './components/GraphNavigator.vue'
import FeedbackAnalysisPanel from './components/FeedbackAnalysisPanel.vue'
import OptimizeAnalysisPanel from './components/OptimizeAnalysisPanel.vue'
import CreateScenarioDialog from './components/CreateScenarioDialog.vue'
import ImpedimentAnalysisPanel from './components/ImpedimentAnalysisPanel.vue'
import NodeRelationshipMenu from './components/NodeRelationshipMenu.vue'
import CommandBar from './components/CommandBar.vue'
import { OptimistApiError } from './api/client'
import { api } from './api/client'
import type {
  CreateEdgeInput,
  AppendObservationInput,
  CorrectObservationInput,
  CreateNodeInput,
  GraphNode,
  GraphEdge,
  Estimate,
  Evidence,
  EvidenceInput,
  EdgeEstimateSlot,
  EdgeKind,
  InterventionEstimateSlot,
  NodeKind,
  Observation,
  ProjectArchive,
  SetStateEstimateInput,
  SetInterventionEstimateInput,
  SetEdgeEstimateInput,
  SetMeasurementCalibrationInput,
  ScenarioDraft,
  UpdateNodeInput,
  UpdateEdgeInput,
} from './api/types'
import { useWorkbenchStore, type WorkbenchMode } from './stores/workbench'
import {
  useCreateNode,
  useCreateEdge,
  useCreateProject,
  useGraph,
  useImportProject,
  useProject,
  useProjects,
  useSetStateEstimate,
  useUpdateNode,
  useUpdateEdge,
  useDeleteEdge,
  useDeleteNode,
  useAppendObservation,
  useCorrectObservation,
  useSetInterventionEstimate,
  useRemoveInterventionEstimate,
  useCreateEvidence,
  useUpdateEvidence,
  useDeleteEvidence,
  useSetEdgeEstimate,
  useRemoveEdgeEstimate,
  useStructuralAnalysis,
  useScenarios,
  useScenarioAnalysis,
  useCreateScenario,
  useUpdateScenario,
  useImpedimentAnalysis,
  useSetMeasurementCalibration,
  useServerHealth,
} from './composables/useProjectData'
import { edgeKinds, endpointsAreValid } from './domain/edgeAuthoring'
import { simulationReadiness } from './domain/simulationReadiness'
import type { WorkbenchCommand } from './domain/commandBar'
import { commandShortcutLabel } from './domain/platformShortcut'

const store = useWorkbenchStore()
const commandShortcutHint = commandShortcutLabel()
const { mode, search, selectedNodeId, selectedProjectId, setupOnly, visibleKinds } = storeToRefs(store)
const projectsQuery = useProjects()
const healthQuery = useServerHealth()
const projectQuery = useProject(selectedProjectId)
const graph = useGraph(selectedProjectId)
const createProject = useCreateProject()
const createNode = useCreateNode(projectQuery.data)
const createEdge = useCreateEdge(projectQuery.data)
const importProject = useImportProject()
const projectDialogOpen = ref(false)
const nodeDialogOpen = ref(false)
const edgeDialogOpen = ref(false)
const edgeDialogSourceId = ref<string | null>(null)
const edgeDialogKind = ref<EdgeKind | null>(null)
const relationshipMenu = ref<{ sourceId: string; x: number; y: number } | null>(null)
const importDialogOpen = ref(false)
const editNodeDialogOpen = ref(false)
const estimateDialogOpen = ref(false)
const edgeEditDialogOpen = ref(false)
const observationDialogOpen = ref(false)
const correctionDialogOpen = ref(false)
const interventionEstimateDialogOpen = ref(false)
const evidenceDialogOpen = ref(false)
const edgeEstimateDialogOpen = ref(false)
const scenarioDialogOpen = ref(false)
const commandBarOpen = ref(false)
const keyboardDeleteTarget = ref<'node' | 'edge' | null>(null)
const selectedEdge = ref<GraphEdge | null>(null)
const selectedMeasurementEdge = ref<GraphEdge | null>(null)
const selectedObservation = ref<Observation | null>(null)
const selectedInterventionSlot = ref<InterventionEstimateSlot | null>(null)
const selectedEvidence = ref<Evidence | null>(null)
const selectedEdgeEstimateSlot = ref<EdgeEstimateSlot | null>(null)
const selectedFeedbackCycle = ref<number | null>(null)
const selectedScenarioId = ref<string | null>(null)
const scenarioDialogScenario = ref<import('./api/types').Scenario | null>(null)
const selectedCandidateId = ref<string | null>(null)
const selectedImpedimentId = ref<string | null>(null)
const highlightedNodeIds = ref<string[]>([])
const highlightedEdgeIds = ref<string[]>([])
const mutationError = ref<Error | null>(null)
const createProjectOption = '__create_project__'

const projects = computed(() => projectsQuery.data.value ?? [])
const nodes = computed(() => graph.nodes.data.value ?? [])
const edges = computed(() => graph.edges.data.value ?? [])
const visibleNodes = computed(() => nodes.value.filter(store.matches))
const visibleEdges = computed(() => {
  const visible = new Set(visibleNodes.value.map((node) => node.id))
  return edges.value.filter(
    (edge) => visible.has(edge.source) && visible.has(edge.destination),
  )
})
const nodesNeedingSetup = computed(() =>
  visibleNodes.value.filter((node) => simulationReadiness(node).level !== 'ready'),
)
const canCreateRelationship = computed(
  () => nodes.value.some((source) =>
    nodes.value.some((destination) =>
      source.id !== destination.id &&
      edgeKinds.some(({ kind }) =>
        endpointsAreValid(kind, source.payload.kind, destination.payload.kind),
      ),
    ),
  ),
)
const selectedNode = computed<GraphNode | null>(
  () => nodes.value.find((node) => node.id === selectedNodeId.value) ?? null,
)
const selectedNodeEdges = computed(() => selectedNode.value
  ? edges.value.filter((edge) => edge.source === selectedNode.value?.id || edge.destination === selectedNode.value?.id)
  : [],
)
const keyboardDeleteTitle = computed(() => keyboardDeleteTarget.value === 'edge' && selectedEdge.value
  ? `${selectedEdge.value.source} ${selectedEdge.value.payload.kind.replaceAll('_', ' ')} ${selectedEdge.value.destination}`
  : selectedNode.value?.title ?? '',
)
const keyboardDeleteBlocked = computed(() => keyboardDeleteTarget.value === 'node' && selectedNodeEdges.value.length
  ? `Delete ${selectedNodeEdges.value.length} connected relationship${selectedNodeEdges.value.length === 1 ? '' : 's'} first.`
  : null,
)
const relationshipMenuSource = computed<GraphNode | null>(() =>
  nodes.value.find((node) => node.id === relationshipMenu.value?.sourceId) ?? null,
)
const updateNode = useUpdateNode(projectQuery.data, selectedNode)
const setStateEstimate = useSetStateEstimate(projectQuery.data, selectedNode)
const updateEdge = useUpdateEdge(projectQuery.data, selectedEdge)
const setMeasurementCalibration = useSetMeasurementCalibration(projectQuery.data, selectedEdge)
const deleteEdge = useDeleteEdge(projectQuery.data, selectedEdge)
const deleteNode = useDeleteNode(projectQuery.data, selectedNode)
const appendObservation = useAppendObservation(projectQuery.data, selectedMeasurementEdge)
const correctObservation = useCorrectObservation(projectQuery.data, selectedMeasurementEdge)
const setInterventionEstimate = useSetInterventionEstimate(projectQuery.data, selectedNode)
const removeInterventionEstimate = useRemoveInterventionEstimate(projectQuery.data, selectedNode)
const createEvidence = useCreateEvidence(projectQuery.data, selectedNode)
const updateEvidence = useUpdateEvidence(projectQuery.data, selectedNode, selectedEvidence)
const deleteEvidence = useDeleteEvidence(projectQuery.data, selectedNode, selectedEvidence)
const setEdgeEstimate = useSetEdgeEstimate(projectQuery.data, selectedEdge)
const removeEdgeEstimate = useRemoveEdgeEstimate(projectQuery.data, selectedEdge)
const feedbackModeEnabled = computed(() => mode.value === 'feedback')
const projectRevision = computed(() => projectQuery.data.value?.revision)
const structuralAnalysis = useStructuralAnalysis(
  selectedProjectId,
  projectRevision,
  feedbackModeEnabled,
)
const optimizeModeEnabled = computed(() => mode.value === 'optimize')
const scenariosQuery = useScenarios(selectedProjectId, optimizeModeEnabled)
const selectedScenario = computed(() =>
  scenariosQuery.data.value?.find((scenario) => scenario.id === selectedScenarioId.value) ?? null,
)
const selectedScenarioRevision = computed(() => selectedScenario.value?.revision)
const scenarioAnalysis = useScenarioAnalysis(
  selectedProjectId,
  selectedScenarioId,
  selectedScenarioRevision,
  optimizeModeEnabled,
)
const createScenario = useCreateScenario(projectQuery.data)
const updateScenario = useUpdateScenario(projectQuery.data, selectedScenario)
const impedimentsModeEnabled = computed(() => mode.value === 'impediments')
const impedimentAnalysis = useImpedimentAnalysis(
  selectedProjectId,
  projectRevision,
  impedimentsModeEnabled,
)
const optimizePending = computed(() =>
  scenariosQuery.isPending.value ||
  (Boolean(selectedScenarioId.value) &&
    (scenarioAnalysis.isPending.value || scenarioAnalysis.isFetching.value)),
)
const loading = computed(
  () =>
    projectsQuery.isPending.value ||
    (Boolean(selectedProjectId.value) &&
      (graph.nodes.isPending.value || graph.edges.isPending.value)),
)
const commandPending = computed(() => createNode.isPending.value || createEdge.isPending.value)
const error = computed(() =>
  [projectsQuery.error.value, projectQuery.error.value, graph.nodes.error.value, graph.edges.error.value]
    .find(Boolean),
)

const kindOptions: Array<{ kind: NodeKind; label: string; icon: typeof Goal }> = [
  { kind: 'outcome', label: 'Outcomes', icon: Goal },
  { kind: 'metric', label: 'Metrics', icon: Gauge },
  { kind: 'factor', label: 'Factors', icon: Activity },
  { kind: 'intervention', label: 'Interventions', icon: Wrench },
]
const modes: Array<{ id: WorkbenchMode; label: string; available: boolean }> = [
  { id: 'explore', label: 'Explore', available: true },
  { id: 'impediments', label: 'Impediments', available: true },
  { id: 'feedback', label: 'Feedback', available: true },
  { id: 'optimize', label: 'Optimize', available: true },
]

watch(
  projects,
  (next) => {
    if (!selectedProjectId.value && next[0]) store.selectProject(next[0].id)
    if (selectedProjectId.value && !next.some((project) => project.id === selectedProjectId.value)) {
      store.selectProject(next[0]?.id ?? null)
    }
  },
  { immediate: true },
)

watch(visibleNodes, (next) => {
  if (selectedNodeId.value && !next.some((node) => node.id === selectedNodeId.value)) {
    store.selectNode(null)
  }
})

watch([mode, selectedProjectId], () => clearFeedbackSelection())
watch(
  () => scenariosQuery.data.value,
  (scenarios) => {
    if (!scenarios?.length) {
      selectedScenarioId.value = null
      return
    }
    if (!selectedScenarioId.value || !scenarios.some((scenario) => scenario.id === selectedScenarioId.value)) {
      selectedScenarioId.value = scenarios[0]!.id
    }
  },
  { immediate: true },
)
watch([mode, selectedProjectId, selectedScenarioId], () => clearOptimizeSelection())
watch([mode, selectedProjectId], () => clearImpedimentSelection())
watch(selectedProjectId, () => { commandBarOpen.value = false })

function commandShortcut(event: KeyboardEvent) {
  if (!(event.metaKey || event.ctrlKey) || event.key.toLocaleLowerCase() !== 'k') return
  event.preventDefault()
  if (selectedProjectId.value) commandBarOpen.value = true
}

function selectionDeleteShortcut(event: KeyboardEvent) {
  if (event.key !== 'Delete' || event.metaKey || event.ctrlKey || event.altKey) return
  const target = event.target as HTMLElement | null
  if (target?.closest('input, textarea, select, [contenteditable="true"]')) return
  if (edgeEditDialogOpen.value && selectedEdge.value) {
    event.preventDefault()
    edgeEditDialogOpen.value = false
    keyboardDeleteTarget.value = 'edge'
    return
  }
  if (document.querySelector('.dialog-backdrop') || !selectedNode.value) return
  event.preventDefault()
  keyboardDeleteTarget.value = 'node'
}

onMounted(() => {
  document.addEventListener('keydown', commandShortcut)
  document.addEventListener('keydown', selectionDeleteShortcut)
})
onBeforeUnmount(() => {
  document.removeEventListener('keydown', commandShortcut)
  document.removeEventListener('keydown', selectionDeleteShortcut)
})

function selectProject(event: Event) {
  const select = event.target as HTMLSelectElement
  if (select.value === createProjectOption) {
    projectDialogOpen.value = true
    select.value = selectedProjectId.value ?? ''
    return
  }
  store.selectProject(select.value || null)
}

function openRelationshipDialog() {
  edgeDialogSourceId.value = null
  edgeDialogKind.value = null
  edgeDialogOpen.value = true
}

function openNodeRelationshipMenu(event: { nodeId: string; x: number; y: number }) {
  store.selectNode(event.nodeId)
  relationshipMenu.value = { sourceId: event.nodeId, x: event.x, y: event.y }
}

function createRelationshipFromNode(kind: EdgeKind) {
  edgeDialogSourceId.value = relationshipMenu.value?.sourceId ?? null
  edgeDialogKind.value = kind
  relationshipMenu.value = null
  edgeDialogOpen.value = true
}

function edgeElementId(edge: import('./api/types').EdgeIdentity) {
  return `${edge.source}:${edge.kind}:${edge.destination}`
}

function selectFeedbackCycle(
  index: number,
  nodes: string[],
  edges: import('./api/types').EdgeIdentity[],
) {
  selectedFeedbackCycle.value = index
  highlightedNodeIds.value = nodes
  highlightedEdgeIds.value = edges.map(edgeElementId)
  store.selectNode(nodes[0] ?? null)
}

function clearFeedbackSelection() {
  selectedFeedbackCycle.value = null
  highlightedNodeIds.value = []
  highlightedEdgeIds.value = []
}

function selectScenario(id: string) {
  selectedScenarioId.value = id
}

function selectCandidate(id: string, nodes: string[]) {
  selectedCandidateId.value = id
  highlightedNodeIds.value = nodes
  highlightedEdgeIds.value = []
  store.selectNode(id)
}

function clearOptimizeSelection() {
  selectedCandidateId.value = null
  if (mode.value === 'optimize') {
    highlightedNodeIds.value = []
    highlightedEdgeIds.value = []
  }
}

function selectImpediment(factor: string, nodes: string[], edges: import('./api/types').EdgeIdentity[]) {
  selectedImpedimentId.value = factor
  highlightedNodeIds.value = nodes
  highlightedEdgeIds.value = edges.map(edgeElementId)
  store.selectNode(factor)
}

function clearImpedimentSelection() {
  selectedImpedimentId.value = null
  if (mode.value === 'impediments') {
    highlightedNodeIds.value = []
    highlightedEdgeIds.value = []
  }
}

async function submitScenario(scenario: ScenarioDraft) {
  mutationError.value = null
  try {
    const saved = scenarioDialogScenario.value
      ? await updateScenario.mutateAsync(scenario)
      : await createScenario.mutateAsync(scenario)
    selectedScenarioId.value = saved.id
    scenarioDialogOpen.value = false
    scenarioDialogScenario.value = null
  } catch (error) {
    mutationError.value = error as Error
  }
}

function createNewScenario() {
  scenarioDialogScenario.value = null
  scenarioDialogOpen.value = true
}

function editSelectedScenario() {
  if (!selectedScenario.value) return
  scenarioDialogScenario.value = selectedScenario.value
  scenarioDialogOpen.value = true
}

async function submitProject(name: string) {
  mutationError.value = null
  try {
    const project = await createProject.mutateAsync(name)
    store.selectProject(project.id)
    projectDialogOpen.value = false
  } catch (error) {
    mutationError.value = error as Error
  }
}

async function submitNode(input: CreateNodeInput) {
  mutationError.value = null
  try {
    const node = await createNode.mutateAsync(input)
    store.selectNode(node.id)
    nodeDialogOpen.value = false
  } catch (error) {
    mutationError.value = error as Error
  }
}

async function applyWorkbenchCommand(command: WorkbenchCommand) {
  mutationError.value = null
  try {
    if (command.type === 'create_node') {
      const node = await createNode.mutateAsync(command.input)
      mode.value = 'explore'
      store.selectNode(node.id)
    } else if (command.type === 'create_edge') {
      await createEdge.mutateAsync(command.input)
      mode.value = 'explore'
      store.selectNode(command.input.source)
    } else if (command.type === 'select_node') {
      mode.value = 'explore'
      store.selectNode(command.node.id)
    } else {
      mode.value = command.mode
    }
    commandBarOpen.value = false
  } catch (error) {
    mutationError.value = error as Error
  }
}

async function submitEdge(input: CreateEdgeInput) {
  mutationError.value = null
  try {
    await createEdge.mutateAsync(input)
    edgeDialogOpen.value = false
  } catch (error) {
    mutationError.value = error as Error
  }
}

async function exportProject() {
  if (!selectedProjectId.value) return
  mutationError.value = null
  try {
    const archive = await api.exportProject(selectedProjectId.value)
    const blob = new Blob([JSON.stringify(archive, null, 2)], {
      type: 'application/json',
    })
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = `${archive.project.id}-${archive.project.name
      .toLocaleLowerCase()
      .replace(/[^a-z0-9]+/g, '-')}.optimist.json`
    link.click()
    URL.revokeObjectURL(url)
  } catch (error) {
    mutationError.value = error as Error
  }
}

async function submitImport(archive: ProjectArchive, replace: boolean) {
  mutationError.value = null
  try {
    const project = await importProject.mutateAsync({ archive, replace })
    store.selectProject(project.id)
    importDialogOpen.value = false
  } catch (error) {
    mutationError.value = error as Error
  }
}

async function submitNodeEdit(input: UpdateNodeInput) {
  mutationError.value = null
  try {
    await updateNode.mutateAsync(input)
    editNodeDialogOpen.value = false
  } catch (error) {
    mutationError.value = error as Error
  }
}

async function submitStateEstimate(input: SetStateEstimateInput) {
  mutationError.value = null
  try {
    await setStateEstimate.mutateAsync(input)
    estimateDialogOpen.value = false
  } catch (error) {
    mutationError.value = error as Error
  }
}

function editRelationship(edge: GraphEdge) {
  selectedEdge.value = edge
  edgeEditDialogOpen.value = true
}

function editRelationshipById(id: string) {
  const edge = edges.value.find((candidate) => edgeElementId({
    source: candidate.source,
    kind: candidate.payload.kind,
    destination: candidate.destination,
  }) === id)
  if (edge) editRelationship(edge)
}

async function submitEdgeEdit(input: UpdateEdgeInput) {
  mutationError.value = null
  try {
    await updateEdge.mutateAsync(input)
    edgeEditDialogOpen.value = false
  } catch (error) {
    mutationError.value = error as Error
  }
}

async function submitMeasurementCalibration(input: SetMeasurementCalibrationInput) {
  mutationError.value = null
  try {
    selectedEdge.value = await setMeasurementCalibration.mutateAsync(input)
  } catch (error) {
    mutationError.value = error as Error
  }
}

async function submitEdgeDelete() {
  mutationError.value = null
  try {
    await deleteEdge.mutateAsync()
    edgeEditDialogOpen.value = false
    selectedEdge.value = null
  } catch (error) {
    mutationError.value = error as Error
  }
}

async function submitNodeDelete() {
  mutationError.value = null
  try {
    await deleteNode.mutateAsync()
    store.selectNode(null)
  } catch (error) {
    mutationError.value = error as Error
  }
}

async function confirmKeyboardDelete() {
  if (keyboardDeleteTarget.value === 'edge') await submitEdgeDelete()
  if (keyboardDeleteTarget.value === 'node') await submitNodeDelete()
  if (!mutationError.value) keyboardDeleteTarget.value = null
}

function observe(edge: GraphEdge) {
  selectedMeasurementEdge.value = edge
  observationDialogOpen.value = true
}

async function submitObservation(input: AppendObservationInput) {
  mutationError.value = null
  try {
    await appendObservation.mutateAsync(input)
    observationDialogOpen.value = false
    selectedMeasurementEdge.value = null
  } catch (error) {
    mutationError.value = error as Error
  }
}

function correct(edge: GraphEdge, observation: Observation) {
  selectedMeasurementEdge.value = edge
  selectedObservation.value = observation
  correctionDialogOpen.value = true
}

async function submitCorrection(input: CorrectObservationInput) {
  mutationError.value = null
  try {
    await correctObservation.mutateAsync(input)
    correctionDialogOpen.value = false
    selectedMeasurementEdge.value = null
    selectedObservation.value = null
  } catch (error) {
    mutationError.value = error as Error
  }
}

function editInterventionEstimate(slot: InterventionEstimateSlot) {
  selectedInterventionSlot.value = slot
  interventionEstimateDialogOpen.value = true
}

async function submitInterventionEstimate(input: SetInterventionEstimateInput) {
  mutationError.value = null
  try {
    await setInterventionEstimate.mutateAsync(input)
    interventionEstimateDialogOpen.value = false
    selectedInterventionSlot.value = null
  } catch (error) {
    mutationError.value = error as Error
  }
}

async function submitInterventionEstimateRemove(estimate: Estimate) {
  mutationError.value = null
  try {
    await removeInterventionEstimate.mutateAsync(estimate)
    interventionEstimateDialogOpen.value = false
    selectedInterventionSlot.value = null
  } catch (error) {
    mutationError.value = error as Error
  }
}

function editEvidence(evidence: Evidence | null) {
  selectedEvidence.value = evidence
  evidenceDialogOpen.value = true
}

async function submitEvidence(input: EvidenceInput) {
  mutationError.value = null
  try {
    if (selectedEvidence.value) await updateEvidence.mutateAsync(input)
    else await createEvidence.mutateAsync(input)
    evidenceDialogOpen.value = false
    selectedEvidence.value = null
  } catch (error) {
    mutationError.value = error as Error
  }
}

async function submitEvidenceDelete() {
  mutationError.value = null
  try {
    await deleteEvidence.mutateAsync()
    evidenceDialogOpen.value = false
    selectedEvidence.value = null
  } catch (error) {
    mutationError.value = error as Error
  }
}

function editEdgeEstimate(slot: EdgeEstimateSlot) {
  selectedEdgeEstimateSlot.value = slot
  edgeEstimateDialogOpen.value = true
}

async function submitEdgeEstimate(input: SetEdgeEstimateInput) {
  mutationError.value = null
  try {
    selectedEdge.value = await setEdgeEstimate.mutateAsync(input)
    edgeEstimateDialogOpen.value = false
    selectedEdgeEstimateSlot.value = null
  } catch (error) {
    mutationError.value = error as Error
  }
}

async function submitEdgeEstimateRemove(estimate: Estimate) {
  mutationError.value = null
  try {
    selectedEdge.value = await removeEdgeEstimate.mutateAsync(estimate)
    edgeEstimateDialogOpen.value = false
    selectedEdgeEstimateSlot.value = null
  } catch (error) {
    mutationError.value = error as Error
  }
}

function errorMessage(value: Error | null) {
  return value instanceof OptimistApiError ? value.message : value?.message
}

function retry() {
  void projectsQuery.refetch()
  void projectQuery.refetch()
  void graph.nodes.refetch()
  void graph.edges.refetch()
}
</script>

<template>
  <main class="workbench-shell">
    <header class="app-header">
      <div class="brand-block">
        <span class="brand-mark"><Network :size="19" /></span>
        <div><strong>Optimist</strong><span>Workbench</span></div>
      </div>

      <div class="project-switcher">
        <select
          :value="selectedProjectId ?? ''"
          aria-label="Project"
          @change="selectProject"
        >
          <option v-if="!projects.length" value="">No projects</option>
          <option v-for="project in projects" :key="project.id" :value="project.id">
            {{ project.name }}
          </option>
          <option :value="createProjectOption">New project...</option>
        </select>
        <ChevronDown :size="15" />
        <span v-if="projectQuery.data.value" class="revision">r{{ projectQuery.data.value.revision }}</span>
      </div>

      <nav class="mode-tabs" aria-label="Analysis mode">
        <button
          v-for="item in modes"
          :key="item.id"
          type="button"
          :class="{ active: mode === item.id }"
          :aria-pressed="mode === item.id"
          :disabled="!item.available"
          :title="item.available ? undefined : 'Analysis mode not yet available'"
          @click="mode = item.id"
        >
          {{ item.label }}
        </button>
      </nav>

      <div class="header-actions">
        <span
          v-if="healthQuery.data.value?.persistence.state !== 'idle'"
          class="persistence-state"
          :data-state="healthQuery.data.value?.persistence.state"
          :title="healthQuery.data.value?.persistence.error ?? 'Compacting durable model snapshot'"
          aria-live="polite"
        >
          <RefreshCw v-if="healthQuery.data.value?.persistence.state === 'pending'" class="spin" :size="13" />
          <AlertTriangle v-else :size="13" />
          {{ healthQuery.data.value?.persistence.state === 'pending' ? 'Saving model' : 'Persistence degraded' }}
        </span>
        <button type="button" class="command-bar-trigger header-icon" :title="`Command bar (${commandShortcutHint})`" :aria-label="`Open command bar (${commandShortcutHint})`" :disabled="!selectedProjectId" @click="commandBarOpen = true">
          <SquareTerminal :size="16" />
          <kbd>{{ commandShortcutHint }}</kbd>
        </button>
        <button type="button" class="icon-button header-icon" title="Import project" aria-label="Import project" @click="importDialogOpen = true">
          <Upload :size="16" />
        </button>
        <button type="button" class="icon-button header-icon" title="Export project" aria-label="Export project" :disabled="!selectedProjectId" @click="exportProject">
          <Download :size="16" />
        </button>
        <button type="button" class="secondary-button" :disabled="!canCreateRelationship" @click="openRelationshipDialog">
          <Link :size="16" /> Relationship
        </button>
        <button type="button" class="primary-button add-node-button" :disabled="!projectQuery.data.value" @click="nodeDialogOpen = true">
          <Plus :size="17" /> Add node
        </button>
      </div>
    </header>

    <section class="workbench-body">
      <aside class="navigator" aria-label="Graph navigator">
        <div class="search-field">
          <Search :size="16" />
          <input v-model="search" type="search" placeholder="Search graph" aria-label="Search graph" />
        </div>

        <div class="filter-section">
          <span class="section-label">Show</span>
          <button
            type="button"
            class="setup-filter"
            :aria-pressed="setupOnly"
            @click="store.toggleSetupOnly"
          >
            <AlertTriangle :size="14" />
            Needs setup
            <span>{{ nodes.filter((node) => simulationReadiness(node).level !== 'ready').length }}</span>
          </button>
          <button
            v-for="item in kindOptions"
            :key="item.kind"
            type="button"
            class="kind-filter"
            :class="{ muted: !visibleKinds.has(item.kind) }"
            :aria-pressed="visibleKinds.has(item.kind)"
            @click="store.toggleKind(item.kind)"
          >
            <span class="kind-dot" :data-kind="item.kind"><component :is="item.icon" :size="14" /></span>
            {{ item.label }}
            <span>{{ nodes.filter((node) => node.payload.kind === item.kind).length }}</span>
          </button>
        </div>

        <GraphNavigator :nodes="visibleNodes" :selected-node-id="selectedNodeId" @select="store.selectNode" />
      </aside>

      <section class="canvas-panel">
        <div class="canvas-status">
          <span><strong>{{ visibleNodes.length }}</strong> nodes</span>
          <span><strong>{{ visibleEdges.length }}</strong> relationships</span>
          <span v-if="nodesNeedingSetup.length" class="readiness-status"><AlertTriangle :size="11" /><strong>{{ nodesNeedingSetup.length }}</strong> need setup</span>
          <span class="mode-note">{{ mode }}</span>
        </div>

        <div v-if="error" class="state-panel error-state">
          <AlertTriangle :size="24" />
          <h2>Could not load the project</h2>
          <p>{{ errorMessage(error as Error) }}</p>
          <button type="button" class="secondary-button" @click="retry"><RefreshCw :size="16" /> Retry</button>
        </div>
        <div v-else-if="loading" class="state-panel">
          <RefreshCw class="spin" :size="24" />
          <h2>Loading model</h2>
        </div>
        <div v-else-if="!projects.length" class="state-panel empty-state">
          <Network :size="28" />
          <h2>Create your first project</h2>
          <p>A project isolates one system model, its estimates, scenarios, and revision history.</p>
          <button type="button" class="primary-button" @click="projectDialogOpen = true"><Plus :size="17" /> Create project</button>
        </div>
        <div v-else-if="!nodes.length" class="state-panel empty-state">
          <CircleDot :size="28" />
          <h2>Start with a system element</h2>
          <p>Add an outcome, metric, factor, or intervention to begin shaping this model.</p>
          <button type="button" class="primary-button" @click="nodeDialogOpen = true"><Plus :size="17" /> Add node</button>
        </div>
        <GraphCanvas
          v-else
          :nodes="visibleNodes"
          :edges="visibleEdges"
          :selected-node-id="selectedNodeId"
          :highlighted-node-ids="highlightedNodeIds"
          :highlighted-edge-ids="highlightedEdgeIds"
          @select="store.selectNode"
          @edit-edge="editRelationshipById"
          @node-contextmenu="openNodeRelationshipMenu"
        />
      </section>

      <FeedbackAnalysisPanel
        v-if="mode === 'feedback'"
        :analysis="structuralAnalysis.data.value"
        :pending="structuralAnalysis.isPending.value || structuralAnalysis.isFetching.value"
        :error="structuralAnalysis.error.value as Error | null"
        :selected-cycle="selectedFeedbackCycle"
        @select="selectFeedbackCycle"
        @clear="clearFeedbackSelection"
        @retry="structuralAnalysis.refetch()"
      />
      <ImpedimentAnalysisPanel
        v-else-if="mode === 'impediments'"
        :analysis="impedimentAnalysis.data.value"
        :pending="impedimentAnalysis.isPending.value || impedimentAnalysis.isFetching.value"
        :error="impedimentAnalysis.error.value as Error | null"
        :nodes="nodes"
        :selected-factor-id="selectedImpedimentId"
        @select="selectImpediment"
        @retry="impedimentAnalysis.refetch()"
      />
      <OptimizeAnalysisPanel
        v-else-if="mode === 'optimize'"
        :scenarios="scenariosQuery.data.value ?? []"
        :selected-scenario-id="selectedScenarioId"
        :analysis="scenarioAnalysis.data.value"
        :pending="optimizePending"
        :error="(scenariosQuery.error.value ?? scenarioAnalysis.error.value) as Error | null"
        :nodes="nodes"
        :selected-candidate-id="selectedCandidateId"
        @select-scenario="selectScenario"
        @select-candidate="selectCandidate"
        @create="createNewScenario"
        @edit="editSelectedScenario"
        @retry="scenariosQuery.error.value ? scenariosQuery.refetch() : scenarioAnalysis.refetch()"
      />
      <NodeInspector
        v-else
        :node="selectedNode"
        :edges="edges"
        @edit="editNodeDialogOpen = true"
        @estimate="estimateDialogOpen = true"
        @relationship="editRelationship"
        @observe="observe"
        @correct="correct"
        @intervention-estimate="editInterventionEstimate"
        @evidence="editEvidence"
        @delete="submitNodeDelete"
      />
    </section>

    <div v-if="mutationError" class="toast" role="alert">
      <AlertTriangle :size="17" />
      <div><strong>Could not save change</strong><span>{{ errorMessage(mutationError) }}</span></div>
      <button type="button" class="icon-button" aria-label="Dismiss error" @click="mutationError = null">×</button>
    </div>

    <CreateProjectDialog
      :open="projectDialogOpen"
      :pending="createProject.isPending.value"
      @close="projectDialogOpen = false"
      @submit="submitProject"
    />
    <CommandBar
      :open="commandBarOpen"
      :pending="commandPending"
      :nodes="nodes"
      :edges="edges"
      @close="commandBarOpen = false"
      @apply="applyWorkbenchCommand"
    />
    <DeleteSelectionDialog
      :open="keyboardDeleteTarget !== null"
      :pending="deleteEdge.isPending.value || deleteNode.isPending.value"
      :kind="keyboardDeleteTarget === 'edge' ? 'relationship' : 'node'"
      :title="keyboardDeleteTitle"
      :blocked-reason="keyboardDeleteBlocked"
      @close="keyboardDeleteTarget = null"
      @confirm="confirmKeyboardDelete"
    />
    <CreateNodeDialog
      :open="nodeDialogOpen"
      :pending="createNode.isPending.value"
      @close="nodeDialogOpen = false"
      @submit="submitNode"
    />
    <CreateEdgeDialog
      :open="edgeDialogOpen"
      :pending="createEdge.isPending.value"
      :project-id="selectedProjectId"
      :nodes="nodes"
      :initial-source-id="edgeDialogSourceId"
      :initial-kind="edgeDialogKind"
      @close="edgeDialogOpen = false"
      @submit="submitEdge"
    />
    <NodeRelationshipMenu
      :open="relationshipMenu !== null"
      :source="relationshipMenuSource"
      :nodes="nodes"
      :x="relationshipMenu?.x ?? 0"
      :y="relationshipMenu?.y ?? 0"
      @close="relationshipMenu = null"
      @select="createRelationshipFromNode"
    />
    <ImportProjectDialog
      :open="importDialogOpen"
      :pending="importProject.isPending.value"
      :project-ids="projects.map((project) => project.id)"
      @close="importDialogOpen = false"
      @submit="submitImport"
    />
    <EditNodeDialog
      :open="editNodeDialogOpen"
      :pending="updateNode.isPending.value"
      :node="selectedNode"
      @close="editNodeDialogOpen = false"
      @submit="submitNodeEdit"
    />
    <EditStateEstimateDialog
      :open="estimateDialogOpen"
      :pending="setStateEstimate.isPending.value"
      :node="selectedNode"
      :project-id="selectedProjectId"
      :edges="edges"
      @close="estimateDialogOpen = false"
      @submit="submitStateEstimate"
    />
    <EditEdgeDialog
      :open="edgeEditDialogOpen"
      :pending="updateEdge.isPending.value || deleteEdge.isPending.value || setMeasurementCalibration.isPending.value"
      :edge="selectedEdge"
      @close="edgeEditDialogOpen = false"
      @submit="submitEdgeEdit"
      @delete="submitEdgeDelete"
      @estimate="editEdgeEstimate"
      @calibration="submitMeasurementCalibration"
    />
    <AddObservationDialog
      :open="observationDialogOpen"
      :pending="appendObservation.isPending.value"
      :edge="selectedMeasurementEdge"
      :unit="selectedNode?.payload.kind === 'metric' ? selectedNode.payload.properties.unit : ''"
      @close="observationDialogOpen = false"
      @submit="submitObservation"
    />
    <CorrectObservationDialog
      :open="correctionDialogOpen"
      :pending="correctObservation.isPending.value"
      :edge="selectedMeasurementEdge"
      :observation="selectedObservation"
      @close="correctionDialogOpen = false"
      @submit="submitCorrection"
    />
    <EditInterventionEstimateDialog
      :open="interventionEstimateDialogOpen"
      :pending="setInterventionEstimate.isPending.value || removeInterventionEstimate.isPending.value"
      :node="selectedNode"
      :slot="selectedInterventionSlot"
      :project-id="selectedProjectId"
      @close="interventionEstimateDialogOpen = false"
      @submit="submitInterventionEstimate"
      @remove="submitInterventionEstimateRemove"
    />
    <EditEvidenceDialog
      :open="evidenceDialogOpen"
      :pending="createEvidence.isPending.value || updateEvidence.isPending.value || deleteEvidence.isPending.value"
      :node="selectedNode"
      :evidence="selectedEvidence"
      @close="evidenceDialogOpen = false"
      @submit="submitEvidence"
      @delete="submitEvidenceDelete"
    />
    <EditEdgeEstimateDialog
      :open="edgeEstimateDialogOpen"
      :pending="setEdgeEstimate.isPending.value || removeEdgeEstimate.isPending.value"
      :edge="selectedEdge"
      :slot="selectedEdgeEstimateSlot"
      :project-id="selectedProjectId"
      @close="edgeEstimateDialogOpen = false"
      @submit="submitEdgeEstimate"
      @remove="submitEdgeEstimateRemove"
    />
    <CreateScenarioDialog
      :open="scenarioDialogOpen"
      :pending="createScenario.isPending.value || updateScenario.isPending.value"
      :nodes="nodes"
      :scenario="scenarioDialogScenario"
      @close="scenarioDialogOpen = false; scenarioDialogScenario = null"
      @submit="submitScenario"
    />
  </main>
</template>

<style scoped>
.workbench-shell { width: 100%; height: 100vh; min-height: 100vh; display: grid; grid-template-rows: 58px minmax(0, 1fr); overflow: hidden; background: #eef0eb; }
.app-header { display: grid; grid-template-columns: 220px minmax(210px, 1fr) auto auto; align-items: center; gap: 18px; padding: 0 14px; background: #fbfcf9; border-bottom: 1px solid var(--line); min-width: 0; }
.brand-block { display: flex; align-items: center; gap: 10px; min-width: 0; }
.brand-mark { width: 34px; height: 34px; display: grid; place-items: center; color: white; background: var(--green); border-radius: 6px; }
.brand-block div { display: grid; line-height: 1.05; }
.brand-block strong { font-size: 15px; }
.brand-block span:last-child { color: var(--muted); font-size: 11px; margin-top: 3px; text-transform: uppercase; }
.project-switcher { position: relative; display: flex; align-items: center; justify-self: start; min-width: 220px; max-width: 420px; height: 36px; border: 1px solid var(--line); border-radius: 6px; background: white; }
.project-switcher select { appearance: none; width: 100%; height: 100%; border: 0; background: transparent; padding: 0 76px 0 12px; color: var(--ink); font-weight: 600; }
.project-switcher > svg { position: absolute; right: 48px; pointer-events: none; color: var(--muted); }
.revision { position: absolute; right: 8px; padding-left: 8px; border-left: 1px solid var(--line); font: 11px 'IBM Plex Mono', monospace; color: var(--muted); }
.persistence-state { display: inline-flex; align-items: center; gap: 5px; color: var(--muted); font-size: 8px; font-weight: 700; white-space: nowrap; }
.persistence-state[data-state='error'] { color: #9a3e31; }
.mode-tabs { display: flex; align-items: center; height: 100%; }
.mode-tabs button { align-self: stretch; border: 0; border-bottom: 2px solid transparent; background: transparent; color: var(--muted); padding: 0 11px; font-size: 12px; font-weight: 600; }
.mode-tabs button.active { color: var(--green); border-bottom-color: var(--green); }
.mode-tabs button:disabled { opacity: .4; cursor: not-allowed; }
.add-node-button { justify-self: end; }
.header-actions { display: flex; align-items: center; gap: 7px; justify-self: end; }
.header-actions button:disabled { opacity: .42; cursor: not-allowed; }
.header-icon { border: 1px solid var(--line); background: white; }
.command-bar-trigger { min-width: 76px; height: 30px; display: inline-flex; align-items: center; justify-content: center; gap: 7px; padding: 0 7px; border-radius: 4px; color: var(--muted); }
.command-bar-trigger:hover { background: #edf0eb; color: var(--ink); }
.command-bar-trigger kbd { min-width: 39px; padding: 2px 5px; border: 1px solid #c9cec8; border-bottom-width: 2px; border-radius: 4px; background: #f7f9f5; color: #53605a; font: 8px 'IBM Plex Mono', monospace; text-align: center; }
.workbench-body { min-height: 0; display: grid; grid-template-columns: 226px minmax(360px, 1fr) 286px; overflow: hidden; }
.navigator { min-height: 0; padding: 14px 12px; overflow: auto; border-right: 1px solid var(--line); background: var(--surface); }
.canvas-panel { position: relative; min-width: 0; min-height: 0; background-color: #f1f3ee; background-image: radial-gradient(#d0d5ce 0.8px, transparent 0.8px); background-size: 18px 18px; }
.search-field { height: 36px; display: flex; align-items: center; gap: 8px; padding: 0 10px; background: white; border: 1px solid var(--line); border-radius: 6px; color: var(--muted); }
.search-field input { min-width: 0; flex: 1; border: 0; outline: 0; background: transparent; font-size: 12px; }
.filter-section { margin-top: 20px; }
.kind-filter, .setup-filter { width: 100%; height: 34px; margin-top: 4px; display: grid; grid-template-columns: 24px 1fr auto; align-items: center; text-align: left; border: 0; border-radius: 5px; background: transparent; color: var(--ink); font-size: 12px; }
.kind-filter:hover, .kind-filter.muted:hover, .setup-filter:hover { background: #ecefe9; }
.kind-filter > span:last-child, .setup-filter > span:last-child { color: var(--muted); font: 11px 'IBM Plex Mono', monospace; }
.kind-filter.muted { opacity: .42; }
.setup-filter { margin-bottom: 8px; border: 1px solid transparent; color: #795710; }
.setup-filter[aria-pressed='true'] { border-color: #d4b171; background: #fff8e9; }
.canvas-status { position: absolute; top: 12px; left: 14px; z-index: 2; display: flex; gap: 6px; }
.canvas-status span { padding: 5px 8px; border: 1px solid var(--line); border-radius: 5px; background: rgba(255,255,255,.9); color: var(--muted); font-size: 10px; }
.canvas-status strong { color: var(--ink); }
.canvas-status .mode-note { color: var(--green); font-weight: 700; text-transform: capitalize; }
.canvas-status .readiness-status { display: inline-flex; align-items: center; gap: 4px; border-color: #d4b171; background: #fff8e9; color: #795710; }
.canvas-status .readiness-status strong { color: #795710; }
.state-panel { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; padding: 24px; color: var(--muted); }
.state-panel h2 { margin: 12px 0 5px; color: var(--ink); font-size: 18px; }
.state-panel p { margin: 0 0 16px; max-width: 380px; font-size: 12px; line-height: 1.55; }
.error-state svg { color: #a83f31; }
.toast { position: fixed; z-index: 30; right: 18px; bottom: 18px; display: grid; grid-template-columns: auto minmax(180px, 1fr) auto; gap: 10px; align-items: start; width: min(390px, calc(100vw - 32px)); padding: 12px; border: 1px solid #d8a098; border-radius: 7px; background: #fff8f6; color: #8c3429; box-shadow: 0 14px 38px rgba(41, 29, 26, .18); }
.toast div { display: grid; gap: 3px; }
.toast strong { font-size: 11px; }
.toast span { color: #654b46; font-size: 10px; line-height: 1.45; }

@media (max-width: 1000px) {
  .app-header { grid-template-columns: 190px 1fr auto; }
  .mode-tabs { display: none; }
  .workbench-body { grid-template-columns: 200px minmax(320px, 1fr) 250px; }
}

@media (max-width: 760px) {
  .workbench-shell { height: auto; min-height: 100svh; grid-template-rows: auto 1fr; overflow: visible; }
  .app-header { grid-template-columns: 1fr auto; gap: 9px; min-height: 112px; padding: 10px; }
  .project-switcher { grid-column: 1 / -1; grid-row: 2; width: 100%; max-width: none; }
  .header-actions { grid-column: 2; grid-row: 1; }
  .header-actions .secondary-button { display: none; }
  .command-bar-trigger { min-width: 30px; width: 30px; padding: 0; }
  .command-bar-trigger kbd { display: none; }
  .workbench-body { min-height: calc(100svh - 112px); grid-template-columns: 1fr; grid-template-rows: auto minmax(330px, 48svh) auto; overflow: visible; }
  .navigator { border-right: 0; border-bottom: 1px solid var(--line); padding: 10px; overflow: visible; }
  .filter-section { margin-top: 10px; display: grid; grid-template-columns: repeat(4, 1fr); gap: 4px; }
  .filter-section > .section-label { display: none; }
  .kind-filter { margin: 0; grid-template-columns: 22px 1fr; padding: 0 5px; }
  .kind-filter > span:last-child { display: none; }
  .setup-filter { grid-column: 1 / -1; margin: 0 0 4px; }
  .canvas-panel { min-height: 330px; }
}
</style>
