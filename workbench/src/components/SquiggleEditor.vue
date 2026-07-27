<script setup lang="ts">
import { closeBrackets, closeBracketsKeymap } from '@codemirror/autocomplete'
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands'
import { bracketMatching } from '@codemirror/language'
import { EditorState, type Extension } from '@codemirror/state'
import { EditorView, keymap, placeholder as placeholderExtension } from '@codemirror/view'
import { completionKeymap } from '@codemirror/autocomplete'
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { squiggleSupport, type ExpressionScope } from '../domain/squiggleLanguage'

const props = withDefaults(
  defineProps<{
    modelValue: string
    scope?: ExpressionScope
    placeholder?: string
    /** Single-line fields commit on Enter instead of accepting a newline. */
    singleLine?: boolean
    readonly?: boolean
  }>(),
  { placeholder: '', singleLine: false, readonly: false },
)

const emit = defineEmits<{
  'update:modelValue': [string]
  commit: [string]
  cancel: []
}>()

const host = ref<HTMLElement | null>(null)
let view: EditorView | null = null

const empty: ExpressionScope = { builtins: [], quantities: [], locals: [] }

function extensions(): Extension[] {
  const base: Extension[] = [
    history(),
    bracketMatching(),
    closeBrackets(),
    ...squiggleSupport(() => props.scope ?? empty),
    placeholderExtension(props.placeholder),
    EditorView.editable.of(!props.readonly),
    EditorState.readOnly.of(props.readonly),
    EditorView.updateListener.of((update) => {
      if (update.docChanged) emit('update:modelValue', update.state.doc.toString())
    }),
  ]

  // Completion and bracket keys are bound before the defaults so that Enter
  // accepts a suggestion rather than committing the field underneath it.
  const keys = [...closeBracketsKeymap, ...completionKeymap, ...historyKeymap, ...defaultKeymap]
  if (props.singleLine) {
    base.push(
      EditorState.transactionFilter.of((transaction) =>
        transaction.newDoc.lines > 1 ? [] : transaction,
      ),
    )
    keys.unshift(
      {
        key: 'Enter',
        run: () => {
          emit('commit', view?.state.doc.toString() ?? '')
          return true
        },
      },
      {
        key: 'Escape',
        run: () => {
          emit('cancel')
          return true
        },
      },
    )
  }
  base.push(keymap.of(keys))
  return base
}

onMounted(() => {
  if (!host.value) return
  view = new EditorView({
    parent: host.value,
    state: EditorState.create({ doc: props.modelValue, extensions: extensions() }),
  })
})

onBeforeUnmount(() => {
  view?.destroy()
  view = null
})

// Only write back when the value differs, or every keystroke would reset the
// cursor to the end of the document.
watch(
  () => props.modelValue,
  (next) => {
    if (!view || next === view.state.doc.toString()) return
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: next } })
  },
)

defineExpose({
  focus: () => view?.focus(),
})
</script>

<template>
  <div ref="host" class="squiggle" :class="{ single: singleLine, readonly }" />
</template>

<style scoped>
.squiggle {
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--surface-strong);
  overflow: hidden;
}
.squiggle.readonly { background: var(--surface); }
.squiggle :deep(.cm-editor) { font-family: var(--mono); font-size: var(--text-sm); }
.squiggle :deep(.cm-editor.cm-focused) { outline: none; }
.squiggle:focus-within { border-color: var(--green); box-shadow: 0 0 0 2px var(--green-soft); }
.squiggle :deep(.cm-content) { padding: 5px 8px; }
.squiggle.single :deep(.cm-content) { padding: 3px 7px; }
.squiggle :deep(.cm-line) { padding: 0; }
.squiggle :deep(.cm-placeholder) { color: var(--muted); font-style: italic; }
.squiggle :deep(.cm-tooltip-autocomplete) {
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--surface-strong);
  box-shadow: 0 6px 18px rgb(0 0 0 / 12%);
  font-family: var(--mono);
  font-size: var(--text-xs);
}
.squiggle :deep(.cm-tooltip-autocomplete ul li[aria-selected]) {
  background: var(--green-soft);
  color: var(--green);
}
.squiggle :deep(.cm-completionDetail) { color: var(--muted); font-style: normal; margin-left: 8px; }
</style>
