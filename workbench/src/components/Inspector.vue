<script setup lang="ts">
import { computed, ref } from 'vue'

import type { Catalogue, Component, Mutation, Relationship, SystemModel } from '../api/types'
import { useDraft, type Draft } from '../composables/useDraft'
import type { ExpressionScope } from '../domain/squiggleLanguage'
import FieldStatus from './FieldStatus.vue'
import SquiggleField from './SquiggleField.vue'

const props = defineProps<{
  design: string
  model: SystemModel
  catalogue?: Catalogue
  selection: { kind: 'component' | 'relationship'; id: string } | null
  apply: (mutations: Mutation[]) => Promise<unknown>
  /** What the solver last refused, so the component at fault can say so. */
  problem?: { component: string | null; message: string } | null
}>()

const emit = defineEmits<{ close: [] }>()

const component = computed<Component | null>(() =>
  props.selection?.kind === 'component'
    ? (props.model.components.find((entry) => entry.id === props.selection!.id) ?? null)
    : null,
)

const relationship = computed<Relationship | null>(() => {
  if (props.selection?.kind !== 'relationship') return null
  const [from, to] = props.selection.id.split('\u2192')
  return props.model.relationships.find((edge) => edge.from === from && edge.to === to) ?? null
})

const definition = computed(() =>
  component.value ? props.catalogue?.component_types[component.value.type] : undefined,
)

/** What a component is called, falling back to its id while it has no name. */
function nameOf(id: string): string {
  return props.model.components.find((entry) => entry.id === id)?.name || id
}

const sender = computed(() => (relationship.value ? nameOf(relationship.value.from) : ''))
const receiver = computed(() => (relationship.value ? nameOf(relationship.value.to) : ''))

/** Whether the solver's complaint is about the thing on screen. */
const blamed = computed(
  () => !!props.problem?.component && props.problem.component === component.value?.id,
)

/**
 * What a property expression may refer to.
 *
 * Shared quantities and the language's own names. A property is evaluated before
 * any channel exists, so offering channel names here would complete to something
 * that cannot resolve.
 */
const scope = computed<ExpressionScope>(() => ({
  builtins: props.catalogue?.builtins ?? [],
  quantities: props.model.scratchpad.map((entry) => ({
    name: entry.name,
    unit: entry.unit,
    summary: entry.summary,
  })),
  locals: [],
}))

/**
 * Editing state per field.
 *
 * Held outside the render and keyed by what it edits, so a field keeps its
 * unsaved text and its error while the design changes around it.
 */
const drafts = new Map<string, Draft<string>>()

function remembered(key: string, make: () => Draft<string>): Draft<string> {
  const existing = drafts.get(key)
  if (existing) return existing
  const draft = make()
  drafts.set(key, draft)
  return draft
}

function propertyDraft(componentId: string, name: string): Draft<string> {
  return remembered(`${componentId}.${name}`, () =>
    useDraft<string>(
      () => props.model.components.find((c) => c.id === componentId)?.properties[name] ?? '',
      async (expression) => {
        const current = props.model.components.find((c) => c.id === componentId)
        if (!current) return
        await props.apply([
          {
            kind: 'set_component',
            component: { ...current, properties: { ...current.properties, [name]: expression } },
          },
        ])
      },
    ),
  )
}

function nameDraft(componentId: string): Draft<string> {
  return remembered(`${componentId}#name`, () =>
    useDraft<string>(
      () => props.model.components.find((c) => c.id === componentId)?.name ?? '',
      async (name) => {
        const current = props.model.components.find((c) => c.id === componentId)
        if (!current) return
        await props.apply([{ kind: 'set_component', component: { ...current, name } }])
      },
    ),
  )
}

/** One of the wire's own limits. Cleared back to absent so the server default returns. */
function wireDraft(field: 'capacity' | 'bandwidth' | 'latency'): Draft<string> {
  return remembered(`${props.selection?.id}#${field}`, () =>
    useDraft<string>(
      () => relationship.value?.[field] ?? '',
      async (expression) => {
        const edge = relationship.value
        if (!edge) return
        const stated = expression.trim()
        await props.apply([
          {
            kind: 'set_relationship',
            relationship: { ...edge, [field]: stated === '' ? undefined : stated },
          },
        ])
      },
    ),
  )
}

function mutatorDraft(type: string, property: string): Draft<string> {
  return remembered(`${props.selection?.id}!${type}.${property}`, () =>
    useDraft<string>(
      () => relationship.value?.mutators.find((m) => m.type === type)?.properties[property] ?? '',
      async (expression) => {
        const edge = relationship.value
        if (!edge) return
        const mutators = edge.mutators.map((mutator) =>
          mutator.type === type
            ? { ...mutator, properties: { ...mutator.properties, [property]: expression } }
            : mutator,
        )
        await props.apply([{ kind: 'set_relationship', relationship: { ...edge, mutators } }])
      },
    ),
  )
}

/** Properties with nothing in them, which is what the solver refuses first. */
const unfilled = computed(() => {
  const current = component.value
  const declared = definition.value?.properties
  if (!current || !declared) return []
  return Object.entries(declared)
    .filter(([name, property]) => (current.properties[name] ?? property.default ?? '').trim() === '')
    .map(([name]) => name)
})

function remove() {
  if (component.value) void props.apply([{ kind: 'remove_component', id: component.value.id }])
  else if (relationship.value) {
    void props.apply([
      { kind: 'remove_relationship', from: relationship.value.from, to: relationship.value.to },
    ])
  }
  emit('close')
}

function addMutator(type: string) {
  const edge = relationship.value
  if (!edge || edge.mutators.some((mutator) => mutator.type === type)) return
  const properties = Object.fromEntries(
    Object.entries(props.catalogue?.mutators[type]?.properties ?? {}).map(([name, property]) => [
      name,
      property.default ?? '',
    ]),
  )
  void props.apply([
    {
      kind: 'set_relationship',
      relationship: { ...edge, mutators: [...edge.mutators, { type, properties }] },
    },
  ])
}

function removeMutator(type: string) {
  const edge = relationship.value
  if (!edge) return
  void props.apply([
    {
      kind: 'set_relationship',
      relationship: { ...edge, mutators: edge.mutators.filter((m) => m.type !== type) },
    },
  ])
}

const available = computed(() =>
  Object.values(props.catalogue?.mutators ?? {}).filter(
    (mutator) => !relationship.value?.mutators.some((attached) => attached.type === mutator.id),
  ),
)

/**
 * Reordering the stack.
 *
 * A request meets the behaviours from the top down and the answer comes back up
 * through them in reverse, so the order is part of what the design says rather
 * than a presentation detail: a retry above a timeout retries a call that timed
 * out, while one below it never learns the call timed out at all.
 */
const dragging = ref<string | null>(null)
const destination = ref<{ type: string; after: boolean } | null>(null)

function beginMove(event: DragEvent, type: string) {
  dragging.value = type
  destination.value = null
  event.dataTransfer?.setData('text/plain', type)
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
}

function considerMove(event: DragEvent, type: string) {
  if (!dragging.value || dragging.value === type) {
    destination.value = null
    return
  }
  const card = event.currentTarget as HTMLElement
  const bounds = card.getBoundingClientRect()
  destination.value = { type, after: event.clientY > bounds.top + bounds.height / 2 }
  if (event.dataTransfer) event.dataTransfer.dropEffect = 'move'
}

function finishMove() {
  dragging.value = null
  destination.value = null
}

function move(event: DragEvent, target: string) {
  const type = dragging.value ?? event.dataTransfer?.getData('text/plain') ?? ''
  const after = destination.value?.type === target && destination.value.after
  finishMove()
  const edge = relationship.value
  if (!edge || !type || type === target) return
  const moved = edge.mutators.find((mutator) => mutator.type === type)
  const mutators = edge.mutators.filter((mutator) => mutator.type !== type)
  const index = mutators.findIndex((mutator) => mutator.type === target)
  if (!moved || index < 0) return
  mutators.splice(after ? index + 1 : index, 0, moved)
  void props.apply([{ kind: 'set_relationship', relationship: { ...edge, mutators } }])
}
</script>

<template>
  <aside v-if="selection" class="inspector">
    <header>
      <div class="heading">
        <span class="kind">{{ selection.kind }}</span>
        <strong>{{ component?.name || selection.id }}</strong>
      </div>
      <el-button text circle size="small" aria-label="Close" @click="emit('close')">
        <el-icon><i-close /></el-icon>
      </el-button>
    </header>

    <div v-if="component" class="body">
      <!--
        A component with an empty property cannot be solved, and until it is
        filled in every analysis of the whole design fails. Saying so here is
        the difference between a design that looks broken and one that is
        obviously half-finished.
      -->
      <el-alert
        v-if="unfilled.length"
        type="warning"
        :closable="false"
        show-icon
        data-test="unfilled-warning"
        :title="`${unfilled.length} propert${unfilled.length === 1 ? 'y needs' : 'ies need'} a value`"
        :description="`Until ${unfilled.join(', ')} ${unfilled.length === 1 ? 'has' : 'have'} a value, the design cannot be solved.`"
      />
      <el-alert
        v-else-if="blamed"
        type="error"
        :closable="false"
        show-icon
        data-test="component-problem"
        title="The solver refused this component"
        :description="problem!.message"
      />

      <el-form label-position="top" size="small">
        <el-form-item label="Name">
          <div class="row">
            <el-input
              v-model="nameDraft(component.id).value.value"
              data-test="component-name"
              @focus="nameDraft(component.id).focus()"
              @blur="nameDraft(component.id).blur()"
            />
            <FieldStatus
              :state="nameDraft(component.id).state.value"
              :error="nameDraft(component.id).error.value"
              :advice="nameDraft(component.id).advice.value"
              @revert="nameDraft(component.id).revert()"
            />
          </div>
        </el-form-item>
      </el-form>

      <p v-if="definition" class="summary">{{ definition.summary }}</p>
      <el-alert
        v-else
        type="error"
        :closable="false"
        show-icon
        title="Unknown type"
        :description="`Nothing in the catalogue defines '${component.type}', so this design cannot be solved.`"
      />

      <section v-if="definition">
        <h4>Properties</h4>
        <div v-for="(property, name) in definition.properties" :key="name" class="field">
          <label class="label">
            <span class="name">{{ name }}</span>
            <span class="spacer" />
            <el-popover trigger="hover" placement="left" :width="300" :content="property.summary">
              <template #reference>
                <el-icon class="about" tabindex="0" :aria-label="`About ${name}`">
                  <i-info-filled />
                </el-icon>
              </template>
            </el-popover>
            <span class="unit">{{ property.unit }}</span>
          </label>
          <div class="row">
            <SquiggleField
              v-model="propertyDraft(component.id, String(name)).value.value"
              :design="design"
              :scope="scope"
              :unit="property.unit"
              :summary="property.summary"
              :placeholder="property.default ?? 'expression'"
              :data-test="`property-${name}`"
              @focus="propertyDraft(component.id, String(name)).focus()"
              @blur="propertyDraft(component.id, String(name)).blur()"
            />
            <FieldStatus
              :state="propertyDraft(component.id, String(name)).state.value"
              :error="propertyDraft(component.id, String(name)).error.value"
              :advice="propertyDraft(component.id, String(name)).advice.value"
              @revert="propertyDraft(component.id, String(name)).revert()"
            />
          </div>
        </div>
      </section>

      <section v-if="definition && Object.keys(definition.channels).length">
        <h4>Computes</h4>
        <ul class="channels">
          <li v-for="(channel, name) in definition.channels" :key="name">
            <span class="name">{{ name }}</span>
            <span class="unit">{{ channel.unit }}</span>
          </li>
        </ul>
      </section>
    </div>

    <div v-else-if="relationship" class="body">
      <p class="flow">
        <span>{{ sender }}</span>
        <el-icon><i-right /></el-icon>
        <span>{{ receiver }}</span>
      </p>
      <p v-if="relationship.summary" class="summary">{{ relationship.summary }}</p>

      <section>
        <h4>Queue depth</h4>
        <p class="hint">
          How many operations may wait on this wire. A deeper queue rides out longer bursts by
          making callers wait for it. Leave it empty for the default of 100.
        </p>
        <div class="field">
          <div class="row">
            <SquiggleField
              v-model="wireDraft('capacity').value.value"
              :design="design"
              :scope="scope"
              unit="operations"
              placeholder="100"
              data-test="relationship-capacity"
              @focus="wireDraft('capacity').focus()"
              @blur="wireDraft('capacity').blur()"
            />
            <FieldStatus
              :state="wireDraft('capacity').state.value"
              :error="wireDraft('capacity').error.value"
              :advice="wireDraft('capacity').advice.value"
              @revert="wireDraft('capacity').revert()"
            />
          </div>
        </div>
      </section>

      <section>
        <h4>Link speed</h4>
        <p class="hint">
          How fast this wire carries bytes, measured against the request and reply payloads
          together. A stated speed both delays the traffic and caps it, so a link too slow for
          its messages backs up in front of a dependency with room to spare. Leave it empty to
          leave the link unlimited.
        </p>
        <div class="field">
          <div class="row">
            <SquiggleField
              v-model="wireDraft('bandwidth').value.value"
              :design="design"
              :scope="scope"
              unit="bytes per second"
              placeholder="unlimited"
              data-test="relationship-bandwidth"
              @focus="wireDraft('bandwidth').focus()"
              @blur="wireDraft('bandwidth').blur()"
            />
            <FieldStatus
              :state="wireDraft('bandwidth').state.value"
              :error="wireDraft('bandwidth').error.value"
              :advice="wireDraft('bandwidth').advice.value"
              @revert="wireDraft('bandwidth').revert()"
            />
          </div>
        </div>
      </section>

      <section>
        <h4>Distance</h4>
        <p class="hint">
          How long a round trip over this wire takes before anybody does any work. Half of it
          carries the request and half carries the reply. Leave it empty for two things close
          enough that the distance between them does not matter.
        </p>
        <div class="field">
          <div class="row">
            <SquiggleField
              v-model="wireDraft('latency').value.value"
              :design="design"
              :scope="scope"
              unit="seconds, round trip"
              placeholder="0"
              data-test="relationship-latency"
              @focus="wireDraft('latency').focus()"
              @blur="wireDraft('latency').blur()"
            />
            <FieldStatus
              :state="wireDraft('latency').state.value"
              :error="wireDraft('latency').error.value"
              :advice="wireDraft('latency').advice.value"
              @revert="wireDraft('latency').revert()"
            />
          </div>
        </div>
      </section>

      <section>
        <h4>Behaviours</h4>
        <p class="hint">
          A request leaves <strong>{{ sender }}</strong> at the top and passes down through each
          behaviour before <strong>{{ receiver }}</strong> sees it. The answer comes back up the
          same stack in reverse, so order decides what each behaviour is told.
        </p>

        <div class="stack" data-test="behaviour-stack">
          <div class="endpoint">
            <el-icon><i-bottom /></el-icon>
            <span><strong>{{ sender }}</strong> sends</span>
          </div>

          <p v-if="!relationship.mutators.length" class="hint empty">
            Nothing changes the flow along this relationship.
          </p>

          <div
            v-for="mutator in relationship.mutators"
            :key="mutator.type"
            class="mutator"
            :class="{
              dragging: dragging === mutator.type,
              'drop-before': destination?.type === mutator.type && !destination.after,
              'drop-after': destination?.type === mutator.type && destination.after,
            }"
            :data-test="`behaviour-${mutator.type}`"
            @dragover.prevent="considerMove($event, mutator.type)"
            @drop.prevent="move($event, mutator.type)"
          >
            <el-card shadow="never">
              <template #header>
                <div class="mutator-head">
                  <el-tooltip content="Drag to reorder" placement="bottom">
                    <span
                      class="move"
                      draggable="true"
                      role="button"
                      tabindex="0"
                      :aria-label="`Move ${catalogue?.mutators[mutator.type]?.name ?? mutator.type}`"
                      :data-test="`move-behaviour-${mutator.type}`"
                      @dragstart="beginMove($event, mutator.type)"
                      @dragend="finishMove"
                    />
                  </el-tooltip>
                  <strong>{{ catalogue?.mutators[mutator.type]?.name ?? mutator.type }}</strong>
                  <el-button text size="small" @click="removeMutator(mutator.type)">
                    Remove
                  </el-button>
                </div>
              </template>
              <!--
                Clamped rather than shown whole: a behaviour's summary runs to a
                paragraph, and two of them at full height leave no room to drag
                one past the other in a panel this narrow.
              -->
              <el-popover
                trigger="hover"
                placement="left"
                :width="300"
                :content="catalogue?.mutators[mutator.type]?.summary"
              >
                <template #reference>
                  <p class="hint clamped">{{ catalogue?.mutators[mutator.type]?.summary }}</p>
                </template>
              </el-popover>
              <div
                v-for="(property, name) in catalogue?.mutators[mutator.type]?.properties ?? {}"
                :key="name"
                class="field"
              >
                <label class="label">
                  <span class="name">{{ name }}</span>
                  <span class="spacer" />
                  <span class="unit">{{ property.unit }}</span>
                </label>
                <div class="row">
                  <SquiggleField
                    v-model="mutatorDraft(mutator.type, String(name)).value.value"
                    :design="design"
                    :scope="scope"
                    :unit="property.unit"
                    :summary="property.summary"
                    @focus="mutatorDraft(mutator.type, String(name)).focus()"
                    @blur="mutatorDraft(mutator.type, String(name)).blur()"
                  />
                  <FieldStatus
                    :state="mutatorDraft(mutator.type, String(name)).state.value"
                    :error="mutatorDraft(mutator.type, String(name)).error.value"
                    :advice="mutatorDraft(mutator.type, String(name)).advice.value"
                    @revert="mutatorDraft(mutator.type, String(name)).revert()"
                  />
                </div>
              </div>
            </el-card>
          </div>

          <div class="endpoint">
            <el-icon><i-top /></el-icon>
            <span><strong>{{ receiver }}</strong> answers back up the stack</span>
          </div>
        </div>

        <el-select
          v-if="available.length"
          placeholder="Attach a behaviour"
          size="small"
          class="attach"
          data-test="attach-mutator"
          popper-class="pick-mutator"
          @change="addMutator"
        >
          <el-option
            v-for="mutator in available"
            :key="mutator.id"
            :label="mutator.name"
            :value="mutator.id"
          />
        </el-select>
      </section>
    </div>

    <footer>
      <el-popconfirm title="Remove this from the design?" @confirm="remove">
        <template #reference>
          <el-button type="danger" plain size="small" data-test="remove-selected">Remove</el-button>
        </template>
      </el-popconfirm>
    </footer>
  </aside>
</template>

<style scoped>
.inspector {
  width: 360px;
  border-left: 1px solid var(--line);
  background: var(--surface-strong);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--line);
}
.heading { display: flex; flex-direction: column; min-width: 0; }
.kind { font-size: var(--text-2xs); color: var(--muted); text-transform: uppercase; letter-spacing: 0.04em; }
.body { flex: 1; overflow: auto; padding: var(--space-4); }
.body > :deep(.el-alert) { margin-bottom: var(--space-3); }
h4 { font-size: var(--text-xs); text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); margin: var(--space-4) 0 var(--space-2); }
.summary, .hint { color: var(--muted); font-size: var(--text-xs); margin: 0 0 var(--space-2); }
.clamped {
  display: -webkit-box;
  -webkit-line-clamp: 3;
  line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
  cursor: help;
}
.field { margin-bottom: var(--space-3); }
.label { display: flex; align-items: center; gap: var(--space-1); margin-bottom: 3px; }
.label .name { font-family: var(--mono); font-size: var(--text-xs); }
.label .spacer { flex: 1; }
.label .unit { font-family: var(--mono); font-size: var(--text-2xs); color: var(--muted); }
.about { font-size: 12px; color: var(--muted); cursor: pointer; }
.about:hover { color: var(--green); }
.row { display: flex; align-items: flex-start; gap: var(--space-2); }
.row > :first-child { flex: 1; min-width: 0; }
.channels { list-style: none; margin: 0; padding: 0; }
.channels li { display: flex; justify-content: space-between; font-family: var(--mono); font-size: var(--text-xs); padding: 2px 0; color: var(--muted); }
.flow { display: flex; align-items: center; gap: var(--space-2); font-size: var(--text-sm); font-weight: 650; margin: 0 0 var(--space-2); }
.stack { border-left: 2px dashed var(--line); padding-left: var(--space-3); margin-bottom: var(--space-2); }
.endpoint { display: flex; align-items: center; gap: var(--space-2); color: var(--muted); font-size: var(--text-xs); padding: var(--space-1) 0; }
.endpoint strong { color: var(--ink); font-weight: 650; }
.empty { margin: var(--space-2) 0; }
.mutator { margin: var(--space-2) 0; }
.mutator.drop-before { box-shadow: inset 0 2px var(--green); }
.mutator.drop-after { box-shadow: inset 0 -2px var(--green); }
.mutator.dragging { opacity: 0.5; }
.mutator-head { display: flex; align-items: center; gap: var(--space-2); }
.mutator-head strong { flex: 1; min-width: 0; }
.move {
  position: relative;
  width: 14px;
  height: 16px;
  flex: none;
  color: var(--muted);
  cursor: grab;
}
.move::before {
  content: '';
  position: absolute;
  top: 2px;
  left: 3px;
  width: 3px;
  height: 3px;
  border-radius: 50%;
  background: currentColor;
  box-shadow:
    6px 0 currentColor,
    0 5px currentColor,
    6px 5px currentColor,
    0 10px currentColor,
    6px 10px currentColor;
}
.move:active { cursor: grabbing; }
.move:hover { color: var(--ink); }
.attach { width: 100%; }
footer { padding: var(--space-3) var(--space-4); border-top: 1px solid var(--line); }
</style>
