<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref } from 'vue'

import type { Catalogue, Mutation, ScratchpadEntry, SystemModel } from '../api/types'
import { useDraft, type Draft } from '../composables/useDraft'
import type { ExpressionScope } from '../domain/squiggleLanguage'
import FieldStatus from './FieldStatus.vue'
import QuantityPreview from './QuantityPreview.vue'
import SquiggleEditor from './SquiggleEditor.vue'

const props = defineProps<{
  design: string
  model: SystemModel
  catalogue?: Catalogue
  apply: (mutations: Mutation[]) => Promise<unknown>
}>()

/**
 * Which expression is being written, and therefore worth previewing.
 *
 * One at a time, following the focus. A preview beside every row would be a
 * column of charts nobody asked for; beside the row being typed into it is the
 * answer to the question the typing is asking.
 */
const editing = ref<string | null>(null)

/** The row each quantity occupies, which is what the preview is anchored to. */
const rows = new Map<string, HTMLElement>()

function hold(name: string, element: unknown) {
  if (element instanceof HTMLElement) rows.set(name, element)
  else rows.delete(name)
}

/**
 * Where the preview sits, in viewport coordinates.
 *
 * The panel this list lives in scrolls, so it clips anything positioned inside
 * it. A preview meant to hang *outside* the panel is therefore rendered at the
 * document root and placed against the viewport, which is the only frame of
 * reference that no ancestor can crop.
 */
const anchor = ref<{ left: number; top: number } | null>(null)

/** Clearance from the row and from the edges of the window. */
const GAP = 12

/**
 * The preview's width, which has to be known before it has been rendered.
 *
 * Kept in step with the component's own stylesheet by [`settle`], which measures
 * what was actually drawn and corrects this first guess. Without the guess the
 * preview would appear at the wrong edge for one frame and jump.
 */
const PREVIEW_WIDTH = 268

function locate(name: string) {
  const row = rows.get(name)
  if (!row) {
    anchor.value = null
    return
  }
  const rect = row.getBoundingClientRect()
  anchor.value = {
    left: Math.max(GAP, rect.left - PREVIEW_WIDTH - GAP),
    top: Math.min(Math.max(GAP, rect.top), window.innerHeight - GAP),
  }
}

/**
 * Places the preview beside its row, inside the window.
 *
 * Its height depends on what the expression turned out to be — a constant is a
 * line of text, a distribution is a chart — so it cannot be reserved for in
 * advance and is measured instead. Everything is derived from the row afresh
 * rather than adjusted from where it last sat, so that scrolling back and forth
 * returns it to the same place instead of ratcheting it up the window.
 */
function settle() {
  const element = preview.value?.$el
  const row = editing.value ? rows.get(editing.value) : null
  if (!(element instanceof HTMLElement) || !row) return
  const anchored = row.getBoundingClientRect()
  const { width, height } = element.getBoundingClientRect()
  const placed = {
    left: Math.max(GAP, anchored.left - width - GAP),
    top: Math.max(GAP, Math.min(anchored.top, window.innerHeight - height - GAP)),
  }
  if (anchor.value?.left === placed.left && anchor.value.top === placed.top) return
  anchor.value = placed
}

const preview = ref<InstanceType<typeof QuantityPreview> | null>(null)

/**
 * Follows the row for as long as the preview is open.
 *
 * A frame loop rather than scroll and resize listeners. The row moves for more
 * reasons than a listener can enumerate — the panel scrolls, the window
 * resizes, the preview itself grows from a line of text into a chart when the
 * expression resolves, rows above it appear and disappear as the design changes
 * underneath — and a `scroll` event does not bubble, so each scrolling ancestor
 * would need its own listener found and attached and released again.
 *
 * Reading two rectangles per frame, for as long as one field has focus, is
 * cheaper than any of that and cannot be wrong. Nothing is written unless the
 * placement actually changed, so a still page costs no renders.
 */
let frame = 0

function track() {
  frame = requestAnimationFrame(track)
  settle()
}

onBeforeUnmount(() => {
  cancelAnimationFrame(frame)
})

/** Starts editing a quantity, bringing its row and its preview into view. */
function beginEditing(name: string) {
  editing.value = name
  void nextTick(() => {
    const row = rows.get(name)
    if (!row) return
    row.scrollIntoView({ block: 'nearest' })
    locate(name)
    cancelAnimationFrame(frame)
    frame = requestAnimationFrame(track)
  })
}

function endEditing(name: string) {
  if (editing.value !== name) return
  cancelAnimationFrame(frame)
  editing.value = null
  anchor.value = null
}

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
        :ref="(element) => hold(entry.name, element)"
        class="entry"
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
          <SquiggleEditor
            v-model="draftFor(entry.name).value.value"
            :scope="scopeFor(index)"
            :data-test="`quantity-${entry.name}`"
            @focus="draftFor(entry.name).focus(); beginEditing(entry.name)"
            @blur="draftFor(entry.name).blur(); endEditing(entry.name)"
          />
          <FieldStatus
            :state="draftFor(entry.name).state.value"
            :error="draftFor(entry.name).error.value"
            :advice="draftFor(entry.name).advice.value"
            @revert="draftFor(entry.name).revert()"
          />
        </div>
      </li>
    </ul>

    <!--
      Rendered at the document root rather than beside the row it belongs to.
      The panel holding these rows scrolls, and a scrolling box crops whatever
      is positioned inside it — which is every pixel of a preview whose whole
      purpose is to hang outside the panel, over the diagram, where there is
      room for a chart.
    -->
    <Teleport to="body">
      <QuantityPreview
        v-if="editing && anchor"
        ref="preview"
        class="flyout"
        :style="{ left: `${anchor.left}px`, top: `${anchor.top}px` }"
        :design="design"
        :expression="draftFor(editing).value.value"
        :entry="editing"
        :unit="model.scratchpad.find((entry) => entry.name === editing)?.unit"
        :summary="model.scratchpad.find((entry) => entry.name === editing)?.summary"
      />
    </Teleport>

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
          <SquiggleEditor
            v-model="draft.expression"
            :scope="newScope"
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
  grid-template-columns: 148px minmax(0, 1fr);
  gap: var(--space-3);
  padding: var(--space-2) 0;
  border-bottom: 1px solid var(--line);
  align-items: start;
}
.label { display: flex; flex-direction: column; gap: 2px; min-width: 0; padding-top: 3px; }
.name { font-family: var(--mono); font-size: var(--text-sm); overflow-wrap: anywhere; }
.meta { display: flex; align-items: center; gap: var(--space-2); color: var(--muted); }
.unit { font-family: var(--mono); font-size: var(--text-2xs); cursor: help; }
.about, .remove { font-size: 13px; cursor: pointer; }
.about:hover { color: var(--green); opacity: 1; }
.unwritten { opacity: 0.45; }
.remove:hover { color: var(--danger); }
.editor { display: flex; align-items: flex-start; gap: var(--space-2); min-width: 0; }
.editor > :first-child { flex: 1; min-width: 0; }
/*
 * Out to the left, over the diagram. The panel is not wide enough to hold a
 * chart beside an expression, and the diagram behind it is the one thing on
 * screen the author is not looking at while they type.
 *
 * Fixed to the viewport, and placed by script: the panel scrolls, so anything
 * positioned within it is cropped at its edge, which is exactly where this
 * needs to cross.
 */
.flyout { position: fixed; z-index: 2100; }
.hint { color: var(--muted); font-size: var(--text-2xs); margin: 2px 0 0; }
</style>
