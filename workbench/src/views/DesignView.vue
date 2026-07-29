<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import type { Bottleneck, Mutation } from '../api/types'
import Inspector from '../components/Inspector.vue'
import ScaleUnitsPanel from '../components/ScaleUnitsPanel.vue'
import ScratchpadPanel from '../components/ScratchpadPanel.vue'
import SolveProgress from '../components/SolveProgress.vue'
import SystemGraph from '../components/SystemGraph.vue'
import { useAnalysis, useCatalogue, useDesign, useEditDesign } from '../composables/useDesign'
import { glyphFor } from '../domain/componentIcons'
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
const { data: analysis, error: solveError, isFetching } = useAnalysis(design, controls, sequence)

/** What makes two solves cost the same amount of arithmetic. */
const shape = computed(() => [design.value, store.samples, store.horizon].join('/'))

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
 * What each component is closest to exhausting, worst first.
 *
 * Fed into the diagram so a strained component is visible without opening
 * anything, and so stopping on one says which limit it is against. Only the
 * worst decides the colour: a component with one constraint at 30% and another
 * at 200% is in trouble, and averaging would hide it.
 */
const constraints = computed(() => {
  const byComponent: Record<string, Bottleneck[]> = {}
  for (const entry of analysis.value?.bottlenecks ?? []) {
    ;(byComponent[entry.component] ??= []).push(entry)
  }
  return byComponent
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
const mutatorTypes = computed(() => Object.values(catalogue.value?.mutators ?? {}))
const componentIds = computed(() => snapshot.value?.model.components.map((c) => c.id) ?? [])

function glyphOf(type: string) {
  return glyphFor(catalogue.value?.component_types[type]?.icon)
}

/**
 * The properties a type cannot be solved without.
 *
 * Shown on the card because it is the difference between choosing a component
 * and finishing one. A queue wants a depth and a service rate; knowing that
 * before the dialog closes is what stops somebody adding four of them and then
 * discovering eight empty fields.
 */
function needs(type: string): string[] {
  return Object.entries(catalogue.value?.component_types[type]?.properties ?? {})
    .filter(([, property]) => property.default === null || property.default === undefined)
    .map(([name]) => name)
}

/**
 * Chooses a type, and offers its name as the identifier where none is typed.
 *
 * A first component of a kind is nearly always called after that kind, and
 * having to type it is a step that teaches nobody anything.
 */
function chooseType(type: string) {
  chosenType.value = type
  if (!newId.value.trim()) newId.value = nameFor(type)
}

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
const attached = ref<string[]>([])

function toggleMutator(id: string) {
  attached.value = attached.value.includes(id)
    ? attached.value.filter((entry) => entry !== id)
    : [...attached.value, id]
}

async function connect() {
  if (!from.value || !to.value || from.value === to.value) return
  await link(from.value, to.value, attached.value)
  connecting.value = false
  from.value = ''
  to.value = ''
  attached.value = []
}

async function link(source: string, target: string, mutators: string[] = []) {
  if (source === target) return
  await apply([
    {
      kind: 'set_relationship',
      relationship: {
        from: source,
        to: target,
        summary: '',
        mutators: mutators.map((type) => ({ type, properties: {} })),
      },
    },
  ])
  select({ kind: 'relationship', id: `${source}\u2192${target}` })
}

/**
 * A name nobody has used, derived from the type.
 *
 * Placing a component from the diagram should not stop to ask what it is called:
 * the point of the gesture is that it is one gesture. The identifier is a
 * starting point rather than a decision, and the inspector opens on the new
 * component so renaming it is the obvious next thing to do.
 */
function nameFor(type: string): string {
  const taken = new Set(componentIds.value)
  if (!taken.has(type)) return type
  for (let index = 2; ; index += 1) {
    const candidate = `${type}-${index}`
    if (!taken.has(candidate)) return candidate
  }
}

async function addComponentAt(type: string, at: { x: number; y: number }) {
  const id = nameFor(type)
  const definition = catalogue.value?.component_types[type]
  const properties = Object.fromEntries(
    Object.entries(definition?.properties ?? {}).map(([name, property]) => [
      name,
      property.default ?? '',
    ]),
  )
  await apply([
    {
      kind: 'set_component',
      component: { id, name: id, type, properties, position: { x: at.x, y: at.y } },
    },
  ])
  select({ kind: 'component', id })
}

function removeComponent(id: string) {
  if (selection.value?.id === id) select(null)
  void apply([{ kind: 'remove_component', id }])
}

/**
 * Whether the new scale unit dialog is open.
 *
 * Held here rather than in the panel because the panel is only on screen while
 * nothing is selected, and grouping components is a thing somebody thinks of
 * *while* looking at one of them. Clearing the selection brings the panel back,
 * already asking the question.
 */
const groupingUp = ref(false)

function beginGrouping() {
  select(null)
  groupingUp.value = true
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
          <el-button
            data-test="add-scale-unit-toolbar"
            :disabled="!componentIds.length"
            @click="beginGrouping"
          >
            <el-icon><i-box /></el-icon>
            <span>Scale unit</span>
          </el-button>
        </el-button-group>
        <span class="spacer" />

        <SolveProgress :solving="isFetching" :shape="shape" />

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
        :constraints="constraints"
        @select="select"
        @move="place"
        @create="({ type, x, y }) => addComponentAt(type, { x, y })"
        @connect="({ from: source, to: target }) => link(source, target)"
        @remove="({ id }) => removeComponent(id)"
      />
    </div>

    <Inspector
      v-if="selection"
      :design="design"
      :model="snapshot.model"
      :catalogue="catalogue"
      :selection="selection"
      :apply="apply"
      :problem="problem"
      @close="select(null)"
    />
    <aside v-else class="scratch">
      <ScratchpadPanel
        :design="design"
        :model="snapshot.model"
        :catalogue="catalogue"
        :apply="apply"
      />
      <hr />
      <ScaleUnitsPanel
        v-model:adding="groupingUp"
        :design="design"
        :model="snapshot.model"
        :catalogue="catalogue"
        :apply="apply"
      />
    </aside>

    <!--
      A gallery rather than a dropdown. Choosing a component type is choosing
      what a thing *is*, and a list of names in a select asks somebody to know
      the vocabulary before they can browse it. The card says what the type
      models and what it will want to know, so the decision is made before the
      dialog closes rather than discovered in the inspector afterwards.
    -->
    <el-dialog v-model="adding" title="Add a component" width="620px">
      <div class="gallery">
        <button
          v-for="type in types"
          :key="type.id"
          class="type"
          :class="{ chosen: chosenType === type.id }"
          :data-test="`component-type-${type.id}`"
          @click="chooseType(type.id)"
        >
          <el-icon class="glyph"><component :is="glyphOf(type.id)" /></el-icon>
          <span class="name">{{ type.name }}</span>
          <span class="says">{{ type.summary.split('.')[0] }}.</span>
          <span v-if="needs(type.id).length" class="needs">
            needs {{ needs(type.id).join(', ') }}
          </span>
          <span v-else class="needs">nothing to supply</span>
        </button>
      </div>

      <el-form label-position="top" size="small" class="naming">
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

    <el-dialog v-model="connecting" title="Connect two components" width="560px">
      <div class="ends">
        <div class="end">
          <p class="label">From</p>
          <el-select v-model="from" data-test="connect-from" popper-class="pick-from">
            <el-option
              v-for="component in snapshot.model.components"
              :key="component.id"
              :label="component.name || component.id"
              :value="component.id"
            />
          </el-select>
        </div>
        <el-icon class="arrow"><i-right /></el-icon>
        <div class="end">
          <p class="label">To</p>
          <el-select v-model="to" data-test="connect-to-select" popper-class="pick-to">
            <el-option
              v-for="component in snapshot.model.components"
              :key="component.id"
              :label="component.name || component.id"
              :value="component.id"
              :disabled="component.id === from"
            />
          </el-select>
        </div>
      </div>

      <!--
        Behaviours are chosen here rather than found later. A retry policy or a
        timeout is part of what a relationship *is*, and a wire drawn without
        one silently models a caller that waits forever and never tries again.
      -->
      <p class="label behaviours">Behaviours on this relationship</p>
      <div class="mutators">
        <button
          v-for="mutator in mutatorTypes"
          :key="mutator.id"
          class="mutator"
          :class="{ chosen: attached.includes(mutator.id) }"
          :data-test="`mutator-${mutator.id}`"
          @click="toggleMutator(mutator.id)"
        >
          <el-icon class="tick">
            <i-select v-if="attached.includes(mutator.id)" />
            <i-plus v-else />
          </el-icon>
          <span class="name">{{ mutator.name }}</span>
          <span class="says">{{ mutator.summary.split('.')[0] }}.</span>
        </button>
        <p v-if="!mutatorTypes.length" class="none">This catalogue defines no behaviours.</p>
      </div>

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
.scratch hr { margin: var(--space-5) 0; border: none; border-top: 1px solid var(--line); }
.option { display: flex; flex-direction: column; line-height: 1.3; padding: 3px 0; }
.option span { font-size: var(--text-2xs); color: var(--muted); }
.message { margin: 0 0 var(--space-2); font-size: var(--text-xs); }
.advice { margin: 0; padding-left: 1.1em; font-size: var(--text-2xs); color: var(--muted); }

.gallery {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
  gap: var(--space-2);
}
.type {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--space-3);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface);
  text-align: left;
  color: var(--ink);
}
.type:hover { border-color: var(--green); background: var(--surface-strong); }
.type.chosen { border-color: var(--green); background: var(--green-soft); }
.glyph { font-size: 17px; color: var(--muted); }
.type.chosen .glyph { color: var(--green); }
.type .name { font-size: var(--text-sm); font-weight: 650; }
.type .says { font-size: var(--text-2xs); color: var(--muted); line-height: 1.35; }
.type .needs {
  margin-top: 2px;
  font-family: var(--mono);
  font-size: 10px;
  color: var(--muted);
  overflow-wrap: anywhere;
}
.naming { margin-top: var(--space-4); }

.ends { display: flex; align-items: flex-end; gap: var(--space-3); }
.end { flex: 1; min-width: 0; }
.arrow { color: var(--muted); padding-bottom: 6px; }
.label {
  margin: 0 0 var(--space-1);
  font-size: var(--text-2xs);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--muted);
  font-weight: 700;
}
.behaviours { margin-top: var(--space-4); }
.mutators { display: grid; grid-template-columns: repeat(auto-fill, minmax(232px, 1fr)); gap: var(--space-2); }
.mutator {
  display: grid;
  grid-template-columns: auto 1fr;
  grid-template-areas: 'tick name' 'tick says';
  gap: 0 var(--space-2);
  padding: var(--space-2);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface);
  text-align: left;
  color: var(--ink);
}
.mutator:hover { border-color: var(--green); }
.mutator.chosen { border-color: var(--green); background: var(--green-soft); }
.mutator .tick { grid-area: tick; font-size: 12px; color: var(--muted); align-self: center; }
.mutator.chosen .tick { color: var(--green); }
.mutator .name { grid-area: name; font-size: var(--text-sm); font-weight: 650; }
.mutator .says { grid-area: says; font-size: var(--text-2xs); color: var(--muted); line-height: 1.35; }
.none { margin: 0; font-size: var(--text-2xs); color: var(--muted); }
</style>
