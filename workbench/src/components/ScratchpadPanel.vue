<script setup lang="ts">
import { computed, ref } from 'vue'

import type { Catalogue, Mutation, ScratchpadEntry, SystemModel } from '../api/types'
import { useDraft, type Draft } from '../composables/useDraft'
import type { ExpressionScope } from '../domain/squiggleLanguage'
import FieldStatus from './FieldStatus.vue'
import SquiggleField from './SquiggleField.vue'

const props = defineProps<{
  design: string
  model: SystemModel
  catalogue?: Catalogue
  apply: (mutations: Mutation[]) => Promise<unknown>
}>()

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

/**
 * One row's editing state, kept by name.
 *
 * Held outside the render so that a row keeps its unsaved text and its error
 * when the design changes around it.
 */
const drafts = new Map<string, Draft<string>>()

function draftFor(name: string): Draft<string> {
  const existing = drafts.get(name)
  if (existing) return existing
  const draft = useDraft<string>(
    () => props.model.scratchpad.find((entry) => entry.name === name)?.expression ?? '',
    async (expression) => {
      const current = props.model.scratchpad.find((entry) => entry.name === name)
      if (!current) return
      await props.apply([{ kind: 'set_scratchpad_entry', entry: { ...current, expression } }])
    },
  )
  drafts.set(name, draft)
  return draft
}

function remove(name: string) {
  drafts.delete(name)
  void props.apply([{ kind: 'remove_scratchpad_entry', name }])
}

const dragging = ref<string | null>(null)
const destination = ref<{ name: string; after: boolean } | null>(null)

function beginMove(event: DragEvent, name: string) {
  dragging.value = name
  destination.value = null
  event.dataTransfer?.setData('text/plain', name)
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
}

function considerMove(event: DragEvent, name: string) {
  if (!dragging.value || dragging.value === name) {
    destination.value = null
    return
  }
  const row = event.currentTarget as HTMLElement
  const bounds = row.getBoundingClientRect()
  destination.value = { name, after: event.clientY > bounds.top + bounds.height / 2 }
  if (event.dataTransfer) event.dataTransfer.dropEffect = 'move'
}

function finishMove() {
  dragging.value = null
  destination.value = null
}

function move(event: DragEvent, target: string) {
  const name = dragging.value ?? event.dataTransfer?.getData('text/plain') ?? ''
  const after = destination.value?.name === target && destination.value.after
  const remaining = props.model.scratchpad.filter((entry) => entry.name !== name)
  const targetIndex = remaining.findIndex((entry) => entry.name === target)
  const before = after ? (remaining[targetIndex + 1]?.name ?? null) : target
  finishMove()
  if (!name || name === before || targetIndex < 0) return
  void props.apply([{ kind: 'move_scratchpad_entry', name, before }])
}

/**
 * Rewriting what a quantity is for.
 *
 * Unlike the unit, the description carries no meaning the numbers depend on, so
 * it is the one part of a declared quantity that stays open to correction — and
 * the part most likely to need it, since it is written before the design that
 * gave the quantity its purpose exists.
 */
const describing = ref<string | null>(null)
const description = ref('')
const descriptionFailure = ref<string | null>(null)

function describe(name: string) {
  description.value = props.model.scratchpad.find((entry) => entry.name === name)?.summary ?? ''
  descriptionFailure.value = null
  describing.value = name
}

async function saveDescription() {
  const current = props.model.scratchpad.find((entry) => entry.name === describing.value)
  if (!current) return
  descriptionFailure.value = null
  try {
    await props.apply([
      { kind: 'set_scratchpad_entry', entry: { ...current, summary: description.value } },
    ])
    describing.value = null
  } catch (error) {
    descriptionFailure.value = (error as Error).message
  }
}

// Adding a quantity.
const adding = ref(false)
const draft = ref<ScratchpadEntry>({ name: '', expression: '', unit: '1', summary: '' })
const failure = ref<string | null>(null)

const newScope = computed(() => scopeFor(props.model.scratchpad.length))
const taken = computed(() => new Set(props.model.scratchpad.map((entry) => entry.name)))

/** The rule the server enforces, applied here so the reason is visible while typing. */
const nameProblem = computed(() => {
  const name = draft.value.name.trim()
  if (!name) return null
  if (taken.value.has(name)) return 'A quantity already goes by that name.'
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) {
    return 'Start with a letter or underscore, then letters, digits and underscores.'
  }
  return null
})

async function add() {
  const name = draft.value.name.trim()
  if (!name || nameProblem.value) return
  failure.value = null
  try {
    await props.apply([{ kind: 'set_scratchpad_entry', entry: { ...draft.value, name } }])
    draft.value = { name: '', expression: '', unit: '1', summary: '' }
    adding.value = false
  } catch (error) {
    failure.value = (error as Error).message
  }
}
</script>

<template>
  <section class="scratchpad">
    <header>
      <h3>Shared quantities</h3>
      <el-button type="primary" size="small" data-test="add-quantity" @click="adding = true">
        <el-icon><i-plus /></el-icon>
        <span>Add</span>
      </el-button>
    </header>

    <el-empty
      v-if="!model.scratchpad.length"
      description="Everything the design is sized against goes here."
      :image-size="60"
    />

    <ul class="entries">
      <li
        v-for="(entry, index) in model.scratchpad"
        :key="entry.name"
        class="entry"
        :class="{
          dragging: dragging === entry.name,
          'drop-before': destination?.name === entry.name && !destination.after,
          'drop-after': destination?.name === entry.name && destination.after,
        }"
        :data-test="`quantity-${entry.name}`"
        @dragover.prevent="considerMove($event, entry.name)"
        @drop.prevent="move($event, entry.name)"
      >
        <!--
          Name, unit and controls stack on the left so the expression gets the
          full width of the row. The expression is the part that is read and
          rewritten; the rest is settled once and then only referred to.
        -->
        <div class="label">
          <code class="name">{{ entry.name }}</code>
          <div class="meta">
            <!--
              The unit is fixed after declaration. Changing it converts nothing,
              so an edit here would silently reinterpret every number that refers
              to this quantity rather than correct it.
            -->
            <el-tooltip content="Units are fixed once a quantity is declared" placement="bottom">
              <span class="unit">{{ entry.unit || '1' }}</span>
            </el-tooltip>
            <el-popover
              trigger="hover"
              placement="right"
              :width="300"
              :content="entry.summary || 'Nothing written down yet. Click to say what this is.'"
            >
              <template #reference>
                <el-icon
                  class="about"
                  :class="{ unwritten: !entry.summary }"
                  tabindex="0"
                  :aria-label="`Describe ${entry.name}`"
                  :data-test="`describe-${entry.name}`"
                  @click="describe(entry.name)"
                  @keydown.enter.prevent="describe(entry.name)"
                >
                  <i-info-filled />
                </el-icon>
              </template>
            </el-popover>
            <el-popconfirm
              :title="`Remove ${entry.name}?`"
              width="240"
              @confirm="remove(entry.name)"
            >
              <template #reference>
                <el-icon class="remove" tabindex="0" :aria-label="`Remove ${entry.name}`">
                  <i-delete />
                </el-icon>
              </template>
            </el-popconfirm>
          </div>
        </div>

        <div class="editor">
          <SquiggleField
            v-model="draftFor(entry.name).value.value"
            :design="design"
            :scope="scopeFor(index)"
            :entry="entry.name"
            :unit="entry.unit"
            :summary="entry.summary"
            :data-test="`quantity-expression-${entry.name}`"
            @focus="draftFor(entry.name).focus()"
            @blur="draftFor(entry.name).blur()"
          />
          <FieldStatus
            :state="draftFor(entry.name).state.value"
            :error="draftFor(entry.name).error.value"
            :advice="draftFor(entry.name).advice.value"
            @revert="draftFor(entry.name).revert()"
          />
        </div>
        <el-tooltip content="Drag to reorder" placement="bottom">
          <span
            class="move"
            draggable="true"
            role="button"
            tabindex="0"
            :aria-label="`Move ${entry.name}`"
            :data-test="`move-${entry.name}`"
            @dragstart="beginMove($event, entry.name)"
            @dragend="finishMove"
          />
        </el-tooltip>
      </li>
    </ul>

    <el-dialog
      :model-value="describing !== null"
      :title="describing ? `What ${describing} is` : ''"
      width="480px"
      @update:model-value="describing = null"
    >
      <el-form label-position="top" size="small" @submit.prevent="saveDescription">
        <el-form-item label="What this number is">
          <el-input v-model="description" type="textarea" :rows="3" data-test="quantity-description" />
        </el-form-item>
        <el-alert
          v-if="descriptionFailure"
          type="error"
          :closable="false"
          show-icon
          :title="descriptionFailure"
        />
      </el-form>
      <template #footer>
        <el-button size="small" @click="describing = null">Cancel</el-button>
        <el-button type="primary" size="small" data-test="save-description" @click="saveDescription">
          Save
        </el-button>
      </template>
    </el-dialog>

    <el-dialog v-model="adding" title="New shared quantity" width="480px">
      <el-form label-position="top" size="small" @submit.prevent="add">
        <el-form-item label="Name" :error="nameProblem ?? undefined">
          <el-input v-model="draft.name" placeholder="peak_rate" data-test="new-quantity-name" />
        </el-form-item>
        <el-form-item label="Expression">
          <SquiggleField
            v-model="draft.expression"
            :design="design"
            :scope="newScope"
            :unit="draft.unit"
            :summary="draft.summary"
            placeholder="900 * lognormal(0, 0.2)"
            data-test="new-quantity-expression"
          />
        </el-form-item>
        <el-form-item label="Unit">
          <el-input v-model="draft.unit" placeholder="op/s" />
          <p class="hint">Fixed once the quantity exists, so it is worth getting right.</p>
        </el-form-item>
        <el-form-item label="What this number is">
          <el-input v-model="draft.summary" type="textarea" :rows="2" />
        </el-form-item>
        <el-alert v-if="failure" type="error" :closable="false" show-icon :title="failure" />
      </el-form>
      <template #footer>
        <el-button size="small" @click="adding = false">Cancel</el-button>
        <el-button
          type="primary"
          size="small"
          :disabled="!draft.name.trim() || !!nameProblem"
          data-test="save-quantity"
          @click="add"
        >
          Add
        </el-button>
      </template>
    </el-dialog>
  </section>
</template>

<style scoped>
.scratchpad { display: flex; flex-direction: column; gap: var(--space-3); }
header { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); }
h3 { font-size: var(--text-md); margin: 0; }
.entries { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; }
.entry {
  display: grid;
  grid-template-columns: 148px minmax(0, 1fr) 18px;
  gap: var(--space-3);
  padding: var(--space-2) 0;
  border-bottom: 1px solid var(--line);
  align-items: start;
}
.entry.drop-before { box-shadow: inset 0 2px var(--green); }
.entry.drop-after { box-shadow: inset 0 -2px var(--green); }
.entry.dragging { opacity: 0.5; }
.label { display: flex; flex-direction: column; gap: 2px; min-width: 0; padding-top: 3px; }
.name { font-family: var(--mono); font-size: var(--text-sm); overflow-wrap: anywhere; }
.move {
  position: relative;
  width: 18px;
  height: 24px;
  color: var(--muted);
  cursor: grab;
  align-self: center;
}
.move::before {
  content: '';
  position: absolute;
  top: 3px;
  left: 4px;
  width: 3px;
  height: 3px;
  border-radius: 50%;
  background: currentColor;
  box-shadow:
    7px 0 currentColor,
    0 5px currentColor,
    7px 5px currentColor,
    0 10px currentColor,
    7px 10px currentColor,
    0 15px currentColor,
    7px 15px currentColor;
}
.move:active { cursor: grabbing; }
.move:hover { color: var(--ink); }
.meta { display: flex; align-items: center; gap: var(--space-2); color: var(--muted); }
.unit { font-family: var(--mono); font-size: var(--text-2xs); cursor: help; }
.about, .remove { font-size: 13px; cursor: pointer; }
.about:hover { color: var(--green); opacity: 1; }
.unwritten { opacity: 0.45; }
.remove:hover { color: var(--danger); }
.editor { display: flex; align-items: flex-start; gap: var(--space-2); min-width: 0; }
.editor > :first-child { flex: 1; min-width: 0; }
.hint { color: var(--muted); font-size: var(--text-2xs); margin: 2px 0 0; }
</style>
