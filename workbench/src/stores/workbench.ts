import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import type { GraphNode, NodeKind } from '../api/types'
import { simulationReadiness } from '../domain/simulationReadiness'

export type WorkbenchMode = 'explore' | 'impediments' | 'feedback' | 'optimize'

export const useWorkbenchStore = defineStore('workbench', () => {
  const selectedProjectId = ref<string | null>(null)
  const selectedNodeId = ref<string | null>(null)
  /** Scenario the optimize view is reading; addressable, so it lives here. */
  const selectedScenarioId = ref<string | null>(null)
  /** Candidate intervention whose detail the optimize view is showing. */
  const selectedCandidateId = ref<string | null>(null)
  const search = ref('')
  const mode = ref<WorkbenchMode>('explore')
  const setupOnly = ref(false)
  const visibleKinds = ref<Set<NodeKind>>(
    new Set(['outcome', 'metric', 'factor', 'intervention']),
  )

  const normalizedSearch = computed(() => search.value.trim().toLocaleLowerCase())

  function selectProject(id: string | null) {
    selectedProjectId.value = id
    selectedNodeId.value = null
    selectedScenarioId.value = null
    selectedCandidateId.value = null
  }

  /** Opens a scenario, dropping a candidate selected under the previous one. */
  function selectScenario(id: string | null) {
    selectedScenarioId.value = id
    selectedCandidateId.value = null
  }

  function selectCandidate(id: string | null) {
    selectedCandidateId.value = id
  }

  function selectNode(id: string | null) {
    selectedNodeId.value = id
  }

  function toggleKind(kind: NodeKind) {
    const next = new Set(visibleKinds.value)
    if (next.has(kind)) next.delete(kind)
    else next.add(kind)
    visibleKinds.value = next
  }

  function toggleSetupOnly() {
    setupOnly.value = !setupOnly.value
  }

  function matches(node: GraphNode) {
    if (!visibleKinds.value.has(node.payload.kind)) return false
    if (setupOnly.value && simulationReadiness(node).level === 'ready') return false
    if (!normalizedSearch.value) return true
    const query = normalizedSearch.value
    return [node.id, node.name, node.title, ...node.aliases].some((value) =>
      value.toLocaleLowerCase().includes(query),
    )
  }

  return {
    selectedProjectId,
    selectedNodeId,
    selectedScenarioId,
    selectedCandidateId,
    search,
    mode,
    setupOnly,
    visibleKinds,
    selectProject,
    selectScenario,
    selectCandidate,
    selectNode,
    toggleKind,
    toggleSetupOnly,
    matches,
  }
})