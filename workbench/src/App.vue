<script setup lang="ts">
import { computed, ref, watch } from 'vue'
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
  Wrench,
} from '@lucide/vue'
import GraphCanvas from './components/GraphCanvas.vue'
import NodeInspector from './components/NodeInspector.vue'
import CreateProjectDialog from './components/CreateProjectDialog.vue'
import CreateNodeDialog from './components/CreateNodeDialog.vue'
import CreateEdgeDialog from './components/CreateEdgeDialog.vue'
import ImportProjectDialog from './components/ImportProjectDialog.vue'
import EditNodeDialog from './components/EditNodeDialog.vue'
import EditStateEstimateDialog from './components/EditStateEstimateDialog.vue'
import EditEdgeDialog from './components/EditEdgeDialog.vue'
import { OptimistApiError } from './api/client'
import { api } from './api/client'
import type {
  CreateEdgeInput,
  CreateNodeInput,
  GraphNode,
  GraphEdge,
  NodeKind,
  ProjectArchive,
  SetStateEstimateInput,
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
} from './composables/useProjectData'

const store = useWorkbenchStore()
const { mode, search, selectedNodeId, selectedProjectId, visibleKinds } = storeToRefs(store)
const projectsQuery = useProjects()
const projectQuery = useProject(selectedProjectId)
const graph = useGraph(selectedProjectId)
const createProject = useCreateProject()
const createNode = useCreateNode(projectQuery.data)
const createEdge = useCreateEdge(projectQuery.data)
const importProject = useImportProject()
const projectDialogOpen = ref(false)
const nodeDialogOpen = ref(false)
const edgeDialogOpen = ref(false)
const importDialogOpen = ref(false)
const editNodeDialogOpen = ref(false)
const estimateDialogOpen = ref(false)
const edgeEditDialogOpen = ref(false)
const selectedEdge = ref<GraphEdge | null>(null)
const mutationError = ref<Error | null>(null)

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
const canCreateRelationship = computed(
  () =>
    nodes.value.filter((node) =>
      ['factor', 'intervention'].includes(node.payload.kind),
    ).length >= 2,
)
const selectedNode = computed<GraphNode | null>(
  () => nodes.value.find((node) => node.id === selectedNodeId.value) ?? null,
)
const updateNode = useUpdateNode(projectQuery.data, selectedNode)
const setStateEstimate = useSetStateEstimate(projectQuery.data, selectedNode)
const updateEdge = useUpdateEdge(projectQuery.data, selectedEdge)
const deleteEdge = useDeleteEdge(projectQuery.data, selectedEdge)
const loading = computed(
  () =>
    projectsQuery.isPending.value ||
    (Boolean(selectedProjectId.value) &&
      (graph.nodes.isPending.value || graph.edges.isPending.value)),
)
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
const modes: Array<{ id: WorkbenchMode; label: string }> = [
  { id: 'explore', label: 'Explore' },
  { id: 'impediments', label: 'Impediments' },
  { id: 'feedback', label: 'Feedback' },
  { id: 'optimize', label: 'Optimize' },
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

async function submitEdgeEdit(input: UpdateEdgeInput) {
  mutationError.value = null
  try {
    await updateEdge.mutateAsync(input)
    edgeEditDialogOpen.value = false
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
          @change="store.selectProject(($event.target as HTMLSelectElement).value || null)"
        >
          <option v-if="!projects.length" value="">No projects</option>
          <option v-for="project in projects" :key="project.id" :value="project.id">
            {{ project.name }}
          </option>
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
          @click="mode = item.id"
        >
          {{ item.label }}
        </button>
      </nav>

      <div class="header-actions">
        <button type="button" class="icon-button header-icon" title="Import project" aria-label="Import project" @click="importDialogOpen = true">
          <Upload :size="16" />
        </button>
        <button type="button" class="icon-button header-icon" title="Export project" aria-label="Export project" :disabled="!selectedProjectId" @click="exportProject">
          <Download :size="16" />
        </button>
        <button type="button" class="secondary-button" :disabled="!canCreateRelationship" @click="edgeDialogOpen = true">
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

        <div class="outline-section">
          <div class="section-title"><span class="section-label">Outline</span><span>{{ visibleNodes.length }}</span></div>
          <div class="node-outline">
            <button
              v-for="node in visibleNodes"
              :key="node.id"
              type="button"
              :class="{ selected: selectedNodeId === node.id }"
              @click="store.selectNode(node.id)"
            >
              <span class="kind-dot" :data-kind="node.payload.kind"><CircleDot :size="13" /></span>
              <span><strong>{{ node.title }}</strong><small>{{ node.name }}</small></span>
              <code>{{ node.id }}</code>
            </button>
          </div>
        </div>
      </aside>

      <section class="canvas-panel">
        <div class="canvas-status">
          <span><strong>{{ visibleNodes.length }}</strong> nodes</span>
          <span><strong>{{ visibleEdges.length }}</strong> relationships</span>
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
          @select="store.selectNode"
        />
      </section>

      <NodeInspector
        :node="selectedNode"
        :edges="edges"
        @edit="editNodeDialogOpen = true"
        @estimate="estimateDialogOpen = true"
        @relationship="editRelationship"
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
    <CreateNodeDialog
      :open="nodeDialogOpen"
      :pending="createNode.isPending.value"
      @close="nodeDialogOpen = false"
      @submit="submitNode"
    />
    <CreateEdgeDialog
      :open="edgeDialogOpen"
      :pending="createEdge.isPending.value"
      :nodes="nodes"
      @close="edgeDialogOpen = false"
      @submit="submitEdge"
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
      @close="estimateDialogOpen = false"
      @submit="submitStateEstimate"
    />
    <EditEdgeDialog
      :open="edgeEditDialogOpen"
      :pending="updateEdge.isPending.value || deleteEdge.isPending.value"
      :edge="selectedEdge"
      @close="edgeEditDialogOpen = false"
      @submit="submitEdgeEdit"
      @delete="submitEdgeDelete"
    />
  </main>
</template>
