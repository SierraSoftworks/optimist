<script setup lang="ts">
import { computed } from 'vue'

import type { Catalogue, Component, Mutation, Relationship, SystemModel } from '../api/types'
import SquiggleEditor from './SquiggleEditor.vue'
import type { ExpressionScope } from '../domain/squiggleLanguage'

const props = defineProps<{
  model: SystemModel
  catalogue?: Catalogue
  selection: { kind: 'component' | 'relationship'; id: string } | null
}>()

const emit = defineEmits<{ edit: [Mutation[]]; close: [] }>()

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

function setProperty(name: string, expression: string) {
  const current = component.value
  if (!current || current.properties[name] === expression) return
  emit('edit', [
    {
      kind: 'set_component',
      component: { ...current, properties: { ...current.properties, [name]: expression } },
    },
  ])
}

function rename(name: string) {
  const current = component.value
  if (!current || current.name === name) return
  emit('edit', [{ kind: 'set_component', component: { ...current, name } }])
}

function remove() {
  if (component.value) emit('edit', [{ kind: 'remove_component', id: component.value.id }])
  else if (relationship.value) {
    emit('edit', [
      {
        kind: 'remove_relationship',
        from: relationship.value.from,
        to: relationship.value.to,
      },
    ])
  }
  emit('close')
}

function setMutator(type: string, property: string, expression: string) {
  const edge = relationship.value
  if (!edge) return
  const mutators = edge.mutators.map((mutator) =>
    mutator.type === type
      ? { ...mutator, properties: { ...mutator.properties, [property]: expression } }
      : mutator,
  )
  emit('edit', [{ kind: 'set_relationship', relationship: { ...edge, mutators } }])
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
  emit('edit', [
    {
      kind: 'set_relationship',
      relationship: { ...edge, mutators: [...edge.mutators, { type, properties }] },
    },
  ])
}

function removeMutator(type: string) {
  const edge = relationship.value
  if (!edge) return
  emit('edit', [
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
        <strong>{{ component?.name ?? selection.id }}</strong>
      </div>
      <el-button text circle size="small" aria-label="Close" @click="emit('close')">
        <el-icon><i-close /></el-icon>
      </el-button>
    </header>

    <div v-if="component" class="body">
      <el-form label-position="top" size="small">
        <el-form-item label="Name">
          <el-input
            :model-value="component.name"
            @update:model-value="rename"
            data-test="component-name"
          />
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
            <span class="unit">{{ property.unit }}</span>
          </label>
          <SquiggleEditor
            :model-value="component.properties[name] ?? property.default ?? ''"
            :scope="scope"
            single-line
            :placeholder="property.default ?? 'expression'"
            :data-test="`property-${name}`"
            @commit="(value) => setProperty(String(name), value)"
          />
          <p class="hint">{{ property.summary }}</p>
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
              <span class="unit">{{ property.unit }}</span>
            </label>
            <SquiggleEditor
              :model-value="mutator.properties[name] ?? ''"
              :scope="scope"
              single-line
              @commit="(value) => setMutator(mutator.type, String(name), value)"
            />
          </div>
        </el-card>

        <el-select
          v-if="available.length"
          placeholder="Attach a behaviour"
          size="small"
          class="attach"
          data-test="attach-mutator"
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
  width: 340px;
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
h4 { font-size: var(--text-xs); text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); margin: var(--space-4) 0 var(--space-2); }
.summary, .hint { color: var(--muted); font-size: var(--text-xs); margin: 0 0 var(--space-2); }
.field { margin-bottom: var(--space-3); }
.label { display: flex; justify-content: space-between; align-items: baseline; margin-bottom: 3px; }
.label .name { font-family: var(--mono); font-size: var(--text-xs); }
.label .unit { font-family: var(--mono); font-size: var(--text-2xs); color: var(--muted); }
.channels { list-style: none; margin: 0; padding: 0; }
.channels li { display: flex; justify-content: space-between; font-family: var(--mono); font-size: var(--text-xs); padding: 2px 0; color: var(--muted); }
.flow { display: flex; align-items: center; gap: var(--space-2); font-family: var(--mono); font-size: var(--text-sm); margin: 0 0 var(--space-2); }
.mutator { margin-bottom: var(--space-2); }
.mutator-head { display: flex; justify-content: space-between; align-items: center; }
.attach { width: 100%; }
footer { padding: var(--space-3) var(--space-4); border-top: 1px solid var(--line); }
</style>
