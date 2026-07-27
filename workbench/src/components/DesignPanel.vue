<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import type { Catalogue, Mutation, ScratchpadEntry, SystemModel } from '../api/types'

const props = defineProps<{ model: SystemModel; catalogue?: Catalogue }>()
const emit = defineEmits<{ edit: [Mutation[]] }>()

/**
 * The entry being edited, held apart from the design.
 *
 * Editing writes into this copy and only reaches the server when the field is
 * committed. Sending on every keystroke would put a partial expression through
 * the solver and fill the feed with states nobody meant to save.
 */
const editing = ref<string | null>(null)
const draft = ref('')

watch(
  () => props.model,
  () => {
    // Somebody else removed what is open here. Closing the editor is better than
    // leaving a field that will fail to save.
    if (editing.value && !props.model.scratchpad.some((e) => e.name === editing.value)) {
      editing.value = null
    }
  },
)

function begin(entry: ScratchpadEntry) {
  editing.value = entry.name
  draft.value = entry.expression
}

function commit(entry: ScratchpadEntry) {
  const expression = draft.value.trim()
  editing.value = null
  if (!expression || expression === entry.expression) return
  emit('edit', [{ kind: 'set_scratchpad_entry', entry: { ...entry, expression } }])
}

const typeOf = computed(() => (id: string) => props.catalogue?.component_types[id])
</script>

<template>
  <div class="design">
    <section>
      <h3>Shared quantities</h3>
      <p class="hint">
        Everything the design is sized against. A proposal works by rebinding these, so a number
        worth arguing about belongs here rather than inside a component.
      </p>
      <ul class="entries">
        <li v-for="entry in model.scratchpad" :key="entry.name">
          <div class="row">
            <span class="name">{{ entry.name }}</span>
            <button
              v-if="editing !== entry.name"
              type="button"
              class="expression"
              @click="begin(entry)"
            >
              {{ entry.expression }}
            </button>
            <input
              v-else
              v-model="draft"
              class="expression editing"
              :aria-label="`Expression for ${entry.name}`"
              autofocus
              @keyup.enter="commit(entry)"
              @keyup.escape="editing = null"
              @blur="commit(entry)"
            />
            <span class="unit">{{ entry.unit }}</span>
          </div>
          <p v-if="entry.summary" class="summary">{{ entry.summary }}</p>
        </li>
      </ul>
      <p v-if="!model.scratchpad.length" class="empty">No shared quantities yet.</p>
    </section>

    <section>
      <h3>Components</h3>
      <ul class="cards">
        <li v-for="component in model.components" :key="component.id" class="card">
          <header>
            <span class="name">{{ component.name }}</span>
            <span class="kind">{{ component.type }}</span>
          </header>
          <p v-if="typeOf(component.type)" class="summary">
            {{ typeOf(component.type)!.summary }}
          </p>
          <p v-else class="unknown">
            This design names a type the catalogue does not define, so it cannot be solved.
          </p>
          <dl class="properties">
            <template v-for="(value, key) in component.properties" :key="key">
              <dt>{{ key }}</dt>
              <dd>{{ value }}</dd>
            </template>
          </dl>
        </li>
      </ul>
    </section>

    <section>
      <h3>Relationships</h3>
      <ul class="flows">
        <li v-for="edge in model.relationships" :key="`${edge.from}-${edge.to}`">
          <span class="from">{{ edge.from }}</span>
          <span class="arrow" aria-hidden="true">&rarr;</span>
          <span class="to">{{ edge.to }}</span>
          <span v-for="mutator in edge.mutators" :key="mutator.type" class="mutator">
            {{ mutator.type }}
          </span>
        </li>
      </ul>
      <p v-if="!model.relationships.length" class="empty">Nothing is connected yet.</p>
    </section>
  </div>
</template>

<style scoped>
.design { display: flex; flex-direction: column; gap: var(--space-6); }
h3 { font-size: var(--text-md); margin: 0 0 var(--space-1); }
.hint, .summary, .empty { color: var(--muted); font-size: var(--text-xs); margin: 0 0 var(--space-3); max-width: 70ch; }
.entries { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--space-2); }
.row { display: flex; align-items: center; gap: var(--space-2); }
.name { font-family: var(--mono); font-size: var(--text-sm); min-width: 15ch; }
.expression {
  font-family: var(--mono);
  font-size: var(--text-sm);
  background: var(--surface);
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  padding: 3px 7px;
  text-align: left;
  flex: 1;
  color: var(--ink);
}
.expression.editing { background: var(--surface-strong); border-color: var(--green); }
.unit { font-size: var(--text-2xs); color: var(--muted); font-family: var(--mono); min-width: 6ch; }
.entries .summary { margin: 0 0 0 calc(15ch + var(--space-2)); }
.cards { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: var(--space-3); }
.card { border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface); padding: var(--space-3); }
.card header { display: flex; justify-content: space-between; align-items: baseline; gap: var(--space-2); }
.card .name { font-weight: 700; min-width: 0; font-family: inherit; }
.kind { font-family: var(--mono); font-size: var(--text-2xs); color: var(--muted); }
.unknown { color: var(--danger); font-size: var(--text-xs); margin: 0 0 var(--space-2); }
.properties { display: grid; grid-template-columns: auto 1fr; gap: 2px var(--space-2); margin: 0; font-size: var(--text-xs); font-family: var(--mono); }
.properties dt { color: var(--muted); }
.properties dd { margin: 0; }
.flows { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--space-1); font-family: var(--mono); font-size: var(--text-sm); }
.flows li { display: flex; align-items: center; gap: var(--space-2); }
.arrow { color: var(--muted); }
.mutator { font-size: var(--text-2xs); background: var(--green-soft); color: var(--green); border-radius: var(--radius-sm); padding: 1px 6px; }
</style>
