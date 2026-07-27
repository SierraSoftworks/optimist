<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import type { Mutation } from '../api/types'
import Inspector from '../components/Inspector.vue'
import ScratchpadPanel from '../components/ScratchpadPanel.vue'
import SystemGraph from '../components/SystemGraph.vue'
import { useAnalysis, useCatalogue, useDesign, useEditDesign } from '../composables/useDesign'
import { readProblem } from '../domain/solverProblem'
import { useWorkbenchStore } from '../stores/workbench'

const props = defineProps<{ design: string; selected?: string }>()

const router = useRouter()
const store = useWorkbenchStore()
const design = computed(() => props.design)

const { data: snapshot } = useDesign(design)
const { data: catalogue } = useCatalogue(design)
const edit = useEditDesign(design)

const controls = computed(() => ({ samples: store.samples, horizon: store.horizon }))
const sequence = computed(() => snapshot.value?.sequence)
const { data: analysis, error: solveError } = useAnalysis(design, controls, sequence)

/**
 * Applies edits and lets the caller see the outcome.
 *
 * Fields await this so each can report its own success or refusal. Swallowing
 * the result here would put every failure in one place at the top of the screen,
 * far from the value that caused it.
 */
function apply(mutations: Mutation[]): Promise<unknown> {
  return edit.mutateAsync(mutations)
}

/** Why the design will not solve, and which component the solver blamed. */
const problem = computed(() => readProblem(solveError.value))

/**
 * How close each component is to a limit, worst constraint winning.
 *
 * Fed into the diagram so that a strained component is visible without opening
 * anything. Only the worst matters: a component with one constraint at 30% and
 * another at 200% is in trouble, and averaging would hide it.
 */
const pressure = computed(() => {
  const worst: Record<string, number> = {}
  for (const entry of analysis.value?.bottlenecks ?? []) {
    worst[entry.component] = Math.max(worst[entry.component] ?? 0, entry.utilisation)
  }
  return worst
})

const selection = computed(() => {
  if (!props.selected) return null
  return props.selected.includes('\u2192')
    ? { kind: 'relationship' as const, id: props.selected }
    : { kind: 'component' as const, id: props.selected }
})

function select(next: { kind: 'component' | 'relationship'; id: string } | null) {
  void router.replace({
    name: 'design',
    params: { design: props.design, selected: next?.id ?? '' },
  })
}

/** Records where a component was dropped, so the arrangement survives a reload. */
function place(move: { id: string; x: number; y: number }) {
  const current = snapshot.value?.model.components.find((entry) => entry.id === move.id)
  if (!current) return
  void apply([
    {
      kind: 'set_component',
      component: { ...current, position: { x: move.x, y: move.y } },
    },
  ])
}

// Adding a component from the catalogue.
const adding = ref(false)
const chosenType = ref('')
const newId = ref('')
const addFailure = ref<string | null>(null)

const types = computed(() => Object.values(catalogue.value?.component_types ?? {}))
const componentIds = computed(() => snapshot.value?.model.components.map((c) => c.id) ?? [])

const idProblem = computed(() => {
  const id = newId.value.trim()
  if (!id) return null
  if (componentIds.value.includes(id)) return 'A component already goes by that name.'
  if (!/^[a-z0-9][a-z0-9-]*$/.test(id)) return 'Use lower-case letters, digits and hyphens.'
  return null
})

async function addComponent() {
  const id = newId.value.trim()
  if (!id || !chosenType.value || idProblem.value) return
  addFailure.value = null
  const definition = catalogue.value?.component_types[chosenType.value]
  // Seeded from the type's defaults, so a property with one is already valid and
  // only the ones that genuinely need a decision are left empty.
  const properties = Object.fromEntries(
    Object.entries(definition?.properties ?? {}).map(([name, property]) => [
      name,
      property.default ?? '',
    ]),
  )
  try {
    await apply([
      { kind: 'set_component', component: { id, name: id, type: chosenType.value, properties } },
    ])
    adding.value = false
    newId.value = ''
    chosenType.value = ''
    select({ kind: 'component', id })
  } catch (error) {
    addFailure.value = (error as Error).message
  }
}

// Connecting two components.
const connecting = ref(false)
const from = ref('')
const to = ref('')

async function connect() {
  if (!from.value || !to.value || from.value === to.value) return
  await apply([
    {
      kind: 'set_relationship',
      relationship: { from: from.value, to: to.value, summary: '', mutators: [] },
    },
  ])
  connecting.value = false
  from.value = ''
  to.value = ''
}

// A selection that no longer exists would leave the inspector describing
// something removed, which is worse than closing it.
watch(
  () => snapshot.value?.model,
  (model) => {
    if (!model || !selection.value) return
    const present =
      selection.value.kind === 'component'
        ? model.components.some((component) => component.id === selection.value!.id)
        : model.relationships.some((edge) => `${edge.from}\u2192${edge.to}` === selection.value!.id)
    if (!present) select(null)
  },
)
</script>

<template>
  <div v-if="snapshot" class="design-view">
    <div class="canvas">
      <div class="toolbar">
        <el-button-group size="small">
          <el-button data-test="add-component" @click="adding = true">
            <el-icon><i-plus /></el-icon>
            <span>Component</span>
          </el-button>
          <el-button
            data-test="add-relationship"
            :disabled="componentIds.length < 2"
            @click="connecting = true"
          >
            <el-icon><i-connection /></el-icon>
            <span>Connect</span>
          </el-button>
        </el-button-group>
        <span class="spacer" />

        <!--
          One line, always in the same place, saying whether the design can be
          solved. A model that will not solve is the normal state while one is
          being built, so this reports rather than alarms — but it never stays
          silent, which is what left the first version looking simply broken.
        -->
        <el-popover v-if="problem" trigger="hover" placement="bottom-end" :width="380">
          <template #reference>
            <span class="verdict bad" data-test="solve-problem">
              <el-icon><i-warning-filled /></el-icon>
              <span>{{ problem.component ? `${problem.component} is incomplete` : 'Will not solve' }}</span>
            </span>
          </template>
          <p class="message">{{ problem.message }}</p>
          <ul v-if="problem.advice.length" class="advice">
            <li v-for="line in problem.advice" :key="line">{{ line }}</li>
          </ul>
        </el-popover>
        <span v-else-if="analysis && !analysis.converged" class="verdict warn">
          <el-icon><i-warning /></el-icon>
          <span>did not settle</span>
        </span>
        <span v-else-if="analysis" class="verdict ok" data-test="solve-ok">
          <el-icon><i-select /></el-icon>
          <span>solves</span>
        </span>
      </div>

      <el-empty
        v-if="!snapshot.model.components.length"
        description="Nothing here yet. Add a component to begin."
        class="blank"
      >
        <el-button type="primary" @click="adding = true">Add a component</el-button>
      </el-empty>
      <SystemGraph
        v-else
        :model="snapshot.model"
        :catalogue="catalogue"
        :selected="selected ?? null"
        :pressure="pressure"
        @select="select"
        @move="place"
      />
    </div>

    <Inspector
      v-if="selection"
      :model="snapshot.model"
      :catalogue="catalogue"
      :selection="selection"
      :apply="apply"
      :problem="problem"
      @close="select(null)"
    />
    <aside v-else class="scratch">
      <ScratchpadPanel :model="snapshot.model" :catalogue="catalogue" :apply="apply" />
    </aside>

    <el-dialog v-model="adding" title="Add a component" width="440px">
      <el-form label-position="top" size="small">
        <el-form-item label="Type">
          <el-select
            v-model="chosenType"
            placeholder="Choose a type"
            data-test="component-type"
            popper-class="pick-component-type"
          >
            <el-option v-for="type in types" :key="type.id" :label="type.name" :value="type.id">
              <div class="option">
                <strong>{{ type.name }}</strong>
                <span>{{ type.summary.split('.')[0] }}</span>
              </div>
            </el-option>
          </el-select>
        </el-form-item>
        <el-form-item label="Identifier" :error="idProblem ?? undefined">
          <el-input v-model="newId" placeholder="api" data-test="component-id" />
        </el-form-item>
        <el-alert v-if="addFailure" type="error" :closable="false" show-icon :title="addFailure" />
      </el-form>
      <template #footer>
        <el-button size="small" @click="adding = false">Cancel</el-button>
        <el-button
          type="primary"
          size="small"
          :disabled="!newId.trim() || !chosenType || !!idProblem"
          data-test="save-component"
          @click="addComponent"
        >
          Add
        </el-button>
      </template>
    </el-dialog>

    <el-dialog v-model="connecting" title="Connect two components" width="420px">
      <el-form label-position="top" size="small">
        <el-form-item label="From">
          <el-select v-model="from" data-test="connect-from" popper-class="pick-from">
            <el-option v-for="id in componentIds" :key="id" :label="id" :value="id" />
          </el-select>
        </el-form-item>
        <el-form-item label="To">
          <el-select v-model="to" data-test="connect-to" popper-class="pick-to">
            <el-option
              v-for="id in componentIds"
              :key="id"
              :label="id"
              :value="id"
              :disabled="id === from"
            />
          </el-select>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button size="small" @click="connecting = false">Cancel</el-button>
        <el-button
          type="primary"
          size="small"
          :disabled="!from || !to || from === to"
          data-test="save-relationship"
          @click="connect"
        >
          Connect
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.design-view { display: flex; flex: 1; min-height: 0; }
.canvas { flex: 1; display: flex; flex-direction: column; min-width: 0; position: relative; }
.toolbar {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px solid var(--line);
  background: var(--surface-strong);
}
.spacer { flex: 1; }
.verdict {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: var(--text-2xs);
  font-weight: 650;
  padding: 3px 9px;
  border-radius: 999px;
  border: 1px solid transparent;
}
.verdict.ok { color: #2f9e69; background: var(--green-soft); }
.verdict.warn { color: var(--caution); background: var(--caution-surface); border-color: var(--caution-line); }
.verdict.bad { color: var(--danger); background: var(--danger-surface); border-color: var(--danger-line); cursor: help; }
.blank { margin: auto; }
.scratch {
  width: 440px;
  border-left: 1px solid var(--line);
  background: var(--surface-strong);
  overflow: auto;
  padding: var(--space-4);
}
.option { display: flex; flex-direction: column; line-height: 1.3; padding: 3px 0; }
.option span { font-size: var(--text-2xs); color: var(--muted); }
.message { margin: 0 0 var(--space-2); font-size: var(--text-xs); }
.advice { margin: 0; padding-left: 1.1em; font-size: var(--text-2xs); color: var(--muted); }
</style>
