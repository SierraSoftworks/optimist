<script setup lang="ts">
import { computed } from 'vue'

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

/** Queue depth on the wire. Cleared back to absent so the server default returns. */
function capacityDraft(): Draft<string> {
  return remembered(`${props.selection?.id}#capacity`, () =>
    useDraft<string>(
      () => relationship.value?.capacity ?? '',
      async (expression) => {
        const edge = relationship.value
        if (!edge) return
        const capacity = expression.trim()
        await props.apply([
          {
            kind: 'set_relationship',
            relationship: { ...edge, capacity: capacity === '' ? undefined : capacity },
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
        <code>{{ relationship.from }}</code>
        <el-icon><i-right /></el-icon>
        <code>{{ relationship.to }}</code>
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
              v-model="capacityDraft().value.value"
              :design="design"
              :scope="scope"
              unit="operations"
              placeholder="100"
              data-test="relationship-capacity"
              @focus="capacityDraft().focus()"
              @blur="capacityDraft().blur()"
            />
            <FieldStatus
              :state="capacityDraft().state.value"
              :error="capacityDraft().error.value"
              :advice="capacityDraft().advice.value"
              @revert="capacityDraft().revert()"
            />
          </div>
        </div>
      </section>

      <section>
        <h4>Behaviours</h4>
        <p v-if="!relationship.mutators.length" class="hint">
          Nothing changes the flow along this relationship.
        </p>
        <el-card
          v-for="mutator in relationship.mutators"
          :key="mutator.type"
          shadow="never"
          class="mutator"
        >
          <template #header>
            <div class="mutator-head">
              <strong>{{ catalogue?.mutators[mutator.type]?.name ?? mutator.type }}</strong>
              <el-button text size="small" @click="removeMutator(mutator.type)">Remove</el-button>
            </div>
          </template>
          <p class="hint">{{ catalogue?.mutators[mutator.type]?.summary }}</p>
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
.flow { display: flex; align-items: center; gap: var(--space-2); font-family: var(--mono); font-size: var(--text-sm); margin: 0 0 var(--space-2); }
.mutator { margin-bottom: var(--space-2); }
.mutator-head { display: flex; justify-content: space-between; align-items: center; }
.attach { width: 100%; }
footer { padding: var(--space-3) var(--space-4); border-top: 1px solid var(--line); }
</style>
