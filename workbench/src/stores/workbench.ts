import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import type { GraphNode, NodeKind } from '../api/types'
import { simulationReadiness } from '../domain/simulationReadiness'

export type WorkbenchMode = 'explore' | 'impediments' | 'feedback' | 'optimize'

export const useWorkbenchStore = defineStore('workbench', () => {
  const selectedProjectId = ref<string | null>(null)
  const selectedNodeId = ref<string | null>(null)
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
    search,
    mode,
    setupOnly,
    visibleKinds,
    selectProject,
    selectNode,
    toggleKind,
    toggleSetupOnly,
    matches,
  }
})