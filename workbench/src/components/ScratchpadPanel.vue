<script setup lang="ts">
import { computed, ref } from 'vue'

import type { Catalogue, Mutation, ScratchpadEntry, SystemModel } from '../api/types'
import type { ExpressionScope } from '../domain/squiggleLanguage'
import SquiggleEditor from './SquiggleEditor.vue'

const props = defineProps<{ model: SystemModel; catalogue?: Catalogue }>()
const emit = defineEmits<{ edit: [Mutation[]] }>()

const adding = ref(false)
const draft = ref<ScratchpadEntry>({ name: '', expression: '', unit: '1', summary: '' })

/**
 * What a shared quantity may refer to.
 *
 * Entries are evaluated in the order they are declared and can only see earlier
 * ones, so completion offers exactly that prefix. Offering the whole list would
 * suggest names that fail to resolve, and the failure would appear at solve time
 * rather than while typing.
 */
function scopeFor(index: number): ExpressionScope {
  return {
    builtins: props.catalogue?.builtins ?? [],
    quantities: props.model.scratchpad.slice(0, index).map((entry) => ({
      name: entry.name,
      unit: entry.unit,
      summary: entry.summary,
    })),
    locals: [
      { name: 't', detail: 'elapsed seconds' },
      { name: 'dt', detail: 'seconds per step' },
    ],
  }
}

const newScope = computed(() => scopeFor(props.model.scratchpad.length))

function update(entry: ScratchpadEntry, changes: Partial<ScratchpadEntry>) {
  const next = { ...entry, ...changes }
  if (JSON.stringify(next) === JSON.stringify(entry)) return
  emit('edit', [{ kind: 'set_scratchpad_entry', entry: next }])
}

function add() {
  const name = draft.value.name.trim()
  if (!name) return
  emit('edit', [{ kind: 'set_scratchpad_entry', entry: { ...draft.value, name } }])
  draft.value = { name: '', expression: '', unit: '1', summary: '' }
  adding.value = false
}

function remove(name: string) {
  emit('edit', [{ kind: 'remove_scratchpad_entry', name }])
}

/** Names already taken, so a new one cannot silently replace an existing entry. */
const taken = computed(() => new Set(props.model.scratchpad.map((entry) => entry.name)))
</script>

<template>
  <section class="scratchpad">
    <header>
      <div>
        <h3>Shared quantities</h3>
        <p class="hint">
          Everything the design is sized against. A proposal works by rebinding these, so a number
          worth arguing about belongs here rather than inside a component.
        </p>
      </div>
      <el-button type="primary" size="small" data-test="add-quantity" @click="adding = true">
        <el-icon><i-plus /></el-icon>
        <span>Add</span>
      </el-button>
    </header>

    <el-empty
      v-if="!model.scratchpad.length && !adding"
      description="No shared quantities yet."
      :image-size="60"
    />

    <ul class="entries">
      <li v-for="(entry, index) in model.scratchpad" :key="entry.name" class="entry">
        <div class="line">
          <code class="name">{{ entry.name }}</code>
          <SquiggleEditor
            class="expression"
            :model-value="entry.expression"
            :scope="scopeFor(index)"
            single-line
            :data-test="`quantity-${entry.name}`"
            @commit="(value) => update(entry, { expression: value })"
          />
          <el-input
            class="unit"
            size="small"
            :model-value="entry.unit"
            @change="(value: string) => update(entry, { unit: value })"
          />
          <el-button
            text
            circle
            size="small"
            :aria-label="`Remove ${entry.name}`"
            @click="remove(entry.name)"
          >
            <el-icon><i-delete /></el-icon>
          </el-button>
        </div>
        <el-input
          class="summary"
          size="small"
          :model-value="entry.summary"
          placeholder="Why this number is what it is"
          @change="(value: string) => update(entry, { summary: value })"
        />
      </li>
    </ul>

    <el-card v-if="adding" shadow="never" class="draft">
      <el-form label-position="top" size="small" @submit.prevent="add">
        <el-form-item label="Name">
          <el-input
            v-model="draft.name"
            placeholder="peak_rate"
            data-test="new-quantity-name"
            autofocus
          />
          <p v-if="taken.has(draft.name.trim())" class="warn">
            That name is taken; adding it would replace the existing quantity.
          </p>
        </el-form-item>
        <el-form-item label="Expression">
          <SquiggleEditor
            v-model="draft.expression"
            :scope="newScope"
            single-line
            placeholder="900 * lognormal(0, 0.2)"
            data-test="new-quantity-expression"
          />
        </el-form-item>
        <el-form-item label="Unit">
          <el-input v-model="draft.unit" placeholder="op/s" />
        </el-form-item>
        <div class="actions">
          <el-button size="small" @click="adding = false">Cancel</el-button>
          <el-button
            type="primary"
            size="small"
            :disabled="!draft.name.trim()"
            data-test="save-quantity"
            @click="add"
          >
            Add
          </el-button>
        </div>
      </el-form>
    </el-card>
  </section>
</template>

<style scoped>
.scratchpad { display: flex; flex-direction: column; gap: var(--space-3); }
header { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--space-4); }
h3 { font-size: var(--text-md); margin: 0; }
.hint { color: var(--muted); font-size: var(--text-xs); margin: 2px 0 0; max-width: 62ch; }
.entries { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--space-3); }
.entry { border-bottom: 1px solid var(--line); padding-bottom: var(--space-3); }
.line { display: flex; align-items: center; gap: var(--space-2); }
.name { font-family: var(--mono); font-size: var(--text-sm); min-width: 16ch; }
.expression { flex: 1; }
.unit { width: 8ch; }
.summary { margin-top: var(--space-1); }
.summary :deep(.el-input__wrapper) { box-shadow: none; padding-left: 0; }
.summary :deep(.el-input__inner) { font-size: var(--text-xs); color: var(--muted); }
.draft { max-width: 460px; }
.actions { display: flex; justify-content: flex-end; gap: var(--space-2); }
.warn { color: var(--caution); font-size: var(--text-2xs); margin: 2px 0 0; }
</style>
