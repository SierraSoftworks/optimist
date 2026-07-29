<script setup lang="ts">
import { ref } from 'vue'

import { useFlyout } from '../composables/useFlyout'
import type { ExpressionScope } from '../domain/squiggleLanguage'
import QuantityPreview from './QuantityPreview.vue'
import SquiggleEditor from './SquiggleEditor.vue'

withDefaults(
  defineProps<{
    modelValue: string
    /** The design the expression is read against, which is what makes it evaluable. */
    design: string
    scope?: ExpressionScope
    placeholder?: string
    singleLine?: boolean
    readonly?: boolean
    /**
     * The shared quantity this field rewrites, where it rewrites one.
     *
     * A quantity may only refer to those declared ahead of it, and a preview
     * that ignored that would show a figure the solver is going to refuse.
     */
    entry?: string | null
    unit?: string
    /** What the number is for, which is what a rewritten expression has to keep being. */
    summary?: string
  }>(),
  { placeholder: '', singleLine: false, readonly: false, entry: null, unit: '', summary: '' },
)

const emit = defineEmits<{
  'update:modelValue': [string]
  commit: [string]
  cancel: []
  focus: []
  blur: []
}>()

/** The preview's width, kept in step with its own stylesheet. */
const PREVIEW_WIDTH = 268

const editor = ref<InstanceType<typeof SquiggleEditor> | null>(null)
const preview = ref<InstanceType<typeof QuantityPreview> | null>(null)

function element(instance: { $el?: unknown } | null): HTMLElement | null {
  return instance?.$el instanceof HTMLElement ? instance.$el : null
}

/**
 * Shown while the field has focus, and only then.
 *
 * A preview beside every expression on screen would be a column of charts nobody
 * asked for; beside the one being typed into it is the answer to the question
 * the typing is asking.
 */
const { at, open, close } = useFlyout(
  () => element(editor.value),
  () => element(preview.value),
  PREVIEW_WIDTH,
)

function began() {
  open()
  emit('focus')
}

function ended() {
  close()
  emit('blur')
}

defineExpose({ focus: () => editor.value?.focus() })
</script>

<template>
  <!--
    A wrapper, because the flyout makes this template a fragment and Vue passes a
    parent's scoped-style marker down to a single root only. Without one, every
    `.row > :first-child { flex: 1 }` at a call site would stop matching.
  -->
  <div class="field">
    <SquiggleEditor
      ref="editor"
      :model-value="modelValue"
      :scope="scope"
      :placeholder="placeholder"
      :single-line="singleLine"
      :readonly="readonly"
      @update:model-value="(value: string) => emit('update:modelValue', value)"
      @commit="(value: string) => emit('commit', value)"
      @cancel="emit('cancel')"
      @focus="began"
      @blur="ended"
    />

    <!--
      Rendered at the document root rather than beside the field it belongs to.
      The panels and dialogs these fields sit in scroll, and a scrolling box
      crops whatever is positioned inside it — which is every pixel of a preview
      whose whole purpose is to hang outside, where there is room for a chart.
    -->
    <Teleport v-if="at" to="body">
      <QuantityPreview
        ref="preview"
        class="flyout"
        :style="{ left: `${at.left}px`, top: `${at.top}px` }"
        :design="design"
        :expression="modelValue"
        :entry="entry"
        :unit="unit"
        :summary="summary"
      />
    </Teleport>
  </div>
</template>

<style scoped>
.field { min-width: 0; }
/* Above dialogs, because the fields inside one need the preview just as much. */
.flyout { position: fixed; z-index: 2100; }
</style>
