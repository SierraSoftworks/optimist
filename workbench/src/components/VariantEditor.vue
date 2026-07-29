<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import type { Catalogue, Intervention, Mutation, SystemModel } from '../api/types'
import type { ExpressionScope } from '../domain/squiggleLanguage'
import SquiggleField from './SquiggleField.vue'

const props = defineProps<{
  design: string
  model: SystemModel
  catalogue?: Catalogue
  /** The variant open for editing, or null to create one. */
  editing: Intervention | null
  apply: (mutations: Mutation[]) => Promise<unknown>
}>()

const emit = defineEmits<{ close: []; saved: [string] }>()

/**
 * What the dialog is showing, held locally rather than read from the prop.
 *
 * The parent forgets which variant was being edited the moment the dialog is
 * dismissed, and a dialog takes a moment to animate away. Reading the prop
 * directly meant the title and every field flipped to the empty "new variant"
 * state on the way out, in full view. The subject is kept until the dialog has
 * actually gone.
 */
const visible = ref(false)
const subject = ref<Intervention | null>(null)
const draft = ref<Intervention>(blank())
const failure = ref<string | null>(null)

function blank(): Intervention {
  return { id: '', name: '', summary: '', overrides: [] }
}

/**
 * Takes a copy of a variant to edit.
 *
 * A copy rather than the thing itself, because the form writes into it as it is
 * filled in and a cancelled edit must leave the design untouched.
 *
 * Written out field by field rather than handed to `structuredClone`, which
 * throws on what it is given here: the variant arrives as a Vue reactive proxy,
 * and the structured clone algorithm refuses proxies. It did throw, silently
 * leaving every field blank, which is worse than either a copy or no copy at
 * all — the dialog said it was editing something and showed nothing.
 */
function copy(variant: Intervention): Intervention {
  return {
    id: variant.id,
    name: variant.name,
    summary: variant.summary,
    overrides: variant.overrides.map((override) => ({ ...override })),
  }
}

watch(
  () => props.editing,
  (variant) => {
    if (!variant) return
    subject.value = variant
    draft.value = copy(variant)
    failure.value = null
    visible.value = true
  },
  { immediate: true },
)

/** Quantities not yet rebound, which are the ones worth offering. */
const spare = computed(() => {
  const used = new Set(draft.value.overrides.map((override) => override.name))
  return props.model.scratchpad.filter((entry) => !used.has(entry.name))
})

function quantity(name: string) {
  return props.model.scratchpad.find((entry) => entry.name === name)
}

/**
 * What a rebound expression may refer to.
 *
 * The same names the quantity itself could see, because a rebind stands exactly
 * where the original expression stood: everything declared ahead of it, and
 * nothing declared after.
 */
function scopeFor(name: string): ExpressionScope {
  const declared = props.model.scratchpad.findIndex((entry) => entry.name === name)
  const ahead = declared < 0 ? props.model.scratchpad : props.model.scratchpad.slice(0, declared)
  return {
    builtins: props.catalogue?.builtins ?? [],
    quantities: ahead.map((entry) => ({
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

const idProblem = computed(() => {
  const id = draft.value.id.trim()
  if (!id) return null
  if (!/^[a-z0-9][a-z0-9-]*$/.test(id)) return 'Use lower-case letters, digits and hyphens.'
  const clash = props.model.interventions.some((entry) => entry.id === id)
  if (clash && subject.value?.id !== id) return 'A variant already goes by that name.'
  return null
})

function slug(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '')
}

function addOverride(name: string) {
  const entry = props.model.scratchpad.find((candidate) => candidate.name === name)
  if (!entry) return
  // Seeded with what the design already says, so the field starts from the value
  // being replaced rather than from nothing.
  draft.value.overrides.push({ name, expression: entry.expression })
}

function removeOverride(name: string) {
  draft.value.overrides = draft.value.overrides.filter((override) => override.name !== name)
}

async function save() {
  const id = draft.value.id.trim() || slug(draft.value.name)
  if (!id || idProblem.value) return
  failure.value = null
  try {
    await props.apply([{ kind: 'set_intervention', intervention: { ...draft.value, id } }])
    emit('saved', id)
    visible.value = false
  } catch (error) {
    failure.value = (error as Error).message
  }
}

/** Runs once the dialog has finished animating away, not when it is dismissed. */
function forget() {
  subject.value = null
  draft.value = blank()
  failure.value = null
  emit('close')
}

defineExpose({
  create: () => {
    subject.value = null
    draft.value = blank()
    failure.value = null
    visible.value = true
  },
})
</script>

<template>
  <el-dialog
    v-model="visible"
    :title="subject ? `Edit ${subject.name}` : 'New variant'"
    width="560px"
    @closed="forget"
  >
    <el-form label-position="top" size="small" @submit.prevent="save">
      <el-form-item label="Name">
        <el-input v-model="draft.name" placeholder="Shed load" data-test="variant-name" />
      </el-form-item>
      <el-form-item label="Identifier" :error="idProblem ?? undefined">
        <el-input
          :model-value="draft.id || slug(draft.name)"
          placeholder="shed-load"
          data-test="variant-id"
          :disabled="!!subject"
          @update:model-value="(value: string) => (draft.id = value)"
        />
        <p class="hint">
          {{ subject ? 'Fixed, because links to this variant use it.' : 'Used in the address bar.' }}
        </p>
      </el-form-item>
      <el-form-item label="What this proposes, and why">
        <el-input v-model="draft.summary" type="textarea" :rows="3" />
      </el-form-item>

      <!--
        A variant is nothing but a set of replacements for shared quantities.
        Saying so in the form is what keeps the model honest: there is no other
        way for a proposal to change a design, so anything worth proposing has to
        be a number the design was sized against.
      -->
      <el-form-item label="Rebinds">
        <div class="overrides">
          <div v-for="override in draft.overrides" :key="override.name" class="override">
            <code class="name">{{ override.name }}</code>
            <SquiggleField
              v-model="override.expression"
              class="expression"
              :design="design"
              :scope="scopeFor(override.name)"
              :entry="override.name"
              :unit="quantity(override.name)?.unit"
              :summary="quantity(override.name)?.summary"
              :data-test="`override-${override.name}`"
            />
            <el-button
              text
              circle
              size="small"
              :aria-label="`Stop rebinding ${override.name}`"
              @click="removeOverride(override.name)"
            >
              <el-icon><i-close /></el-icon>
            </el-button>
          </div>
          <p v-if="!draft.overrides.length" class="hint">
            Nothing is rebound, so this variant would behave exactly like the design.
          </p>
          <el-select
            v-if="spare.length"
            placeholder="Rebind a quantity"
            size="small"
            class="add"
            data-test="add-override"
            popper-class="pick-override"
            @change="addOverride"
          >
            <el-option
              v-for="entry in spare"
              :key="entry.name"
              :label="entry.name"
              :value="entry.name"
            >
              <div class="option">
                <strong>{{ entry.name }}</strong>
                <span>{{ entry.expression }}</span>
              </div>
            </el-option>
          </el-select>
        </div>
      </el-form-item>

      <el-alert v-if="failure" type="error" :closable="false" show-icon :title="failure" />
    </el-form>

    <template #footer>
      <el-button size="small" @click="visible = false">Cancel</el-button>
      <el-button
        type="primary"
        size="small"
        :disabled="!draft.name.trim() || !!idProblem"
        data-test="save-variant"
        @click="save"
      >
        {{ subject ? 'Save' : 'Create' }}
      </el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.hint { color: var(--muted); font-size: var(--text-2xs); margin: 2px 0 0; }
.overrides { width: 100%; display: flex; flex-direction: column; gap: var(--space-2); }
.override { display: flex; align-items: center; gap: var(--space-2); }
.override .name { font-family: var(--mono); font-size: var(--text-xs); min-width: 13ch; }
.expression { flex: 1; min-width: 0; }
.add { width: 100%; }
.option { display: flex; flex-direction: column; line-height: 1.3; padding: 2px 0; }
.option span { font-size: var(--text-2xs); color: var(--muted); font-family: var(--mono); }
</style>
