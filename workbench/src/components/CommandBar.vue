<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { CornerDownLeft, Search, SquareTerminal, X } from '@lucide/vue'
import type { GraphEdge, GraphNode } from '../api/types'
import {
  commandPreview,
  commandSuggestions,
  parseCommand,
  type CommandSuggestion,
  type WorkbenchCommand,
} from '../domain/commandBar'

const props = defineProps<{
  open: boolean
  pending: boolean
  nodes: GraphNode[]
  edges: GraphEdge[]
}>()
const emit = defineEmits<{
  close: []
  apply: [command: WorkbenchCommand]
}>()
const input = ref('')
const inputElement = ref<HTMLInputElement>()
const selectedSuggestion = ref(0)
const result = computed(() => parseCommand(input.value, props.nodes, props.edges))
const suggestions = computed(() => commandSuggestions(input.value, props.nodes))
const preview = computed(() => result.value.command ? commandPreview(result.value.command) : [])

watch(
  () => props.open,
  async (open) => {
    if (!open) return
    input.value = ''
    selectedSuggestion.value = 0
    await nextTick()
    inputElement.value?.focus()
  },
)
watch(suggestions, () => { selectedSuggestion.value = 0 })

function choose(suggestion: CommandSuggestion) {
  input.value = suggestion.value
  void nextTick(() => inputElement.value?.focus())
}

function submit() {
  if (result.value.command && !props.pending) emit('apply', result.value.command)
}

function keys(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    emit('close')
    return
  }
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    if (!suggestions.value.length) return
    event.preventDefault()
    const direction = event.key === 'ArrowDown' ? 1 : -1
    selectedSuggestion.value =
      (selectedSuggestion.value + direction + suggestions.value.length) % suggestions.value.length
    return
  }
  if (event.key === 'Tab' || event.key === 'Enter' && !result.value.command) {
    const suggestion = suggestions.value[selectedSuggestion.value]
    if (!suggestion) return
    event.preventDefault()
    choose(suggestion)
  }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="command-backdrop" @pointerdown.self="emit('close')">
      <form class="command-bar" role="dialog" aria-modal="true" aria-labelledby="command-bar-title" @submit.prevent="submit">
        <header>
          <span><SquareTerminal :size="17" /><strong id="command-bar-title">Command bar</strong></span>
          <button type="button" class="icon-button" aria-label="Close command bar" @click="emit('close')"><X :size="17" /></button>
        </header>
        <label class="command-input">
          <Search :size="17" />
          <input ref="inputElement" v-model="input" aria-label="Command" autocomplete="off" spellcheck="false" placeholder="add factor &quot;Fast feedback&quot;" @keydown="keys" />
        </label>

        <div class="command-body">
          <div v-if="suggestions.length" class="command-suggestions" role="listbox" aria-label="Command suggestions">
            <button
              v-for="(suggestion, index) in suggestions"
              :key="suggestion.value"
              type="button"
              role="option"
              :aria-selected="selectedSuggestion === index"
              @mouseenter="selectedSuggestion = index"
              @click="choose(suggestion)"
            >
              <span><strong>{{ suggestion.label }}</strong><small>{{ suggestion.detail }}</small></span>
              <code>{{ suggestion.value }}</code>
            </button>
          </div>

          <section v-if="preview.length" class="command-preview" aria-label="Command preview">
            <span class="preview-label">Preview</span>
            <dl><div v-for="[label, value] in preview" :key="label"><dt>{{ label }}</dt><dd>{{ value }}</dd></div></dl>
          </section>
          <p class="command-diagnostic" :data-severity="result.diagnostic.severity" :role="result.diagnostic.severity === 'error' ? 'alert' : 'status'">{{ result.diagnostic.message }}</p>
        </div>

        <footer>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="!result.command || pending">
            {{ pending ? 'Applying…' : 'Apply' }} <CornerDownLeft :size="14" />
          </button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>

<style scoped>
.command-backdrop { position: fixed; inset: 0; z-index: 90; display: grid; place-items: start center; padding: min(14vh, 120px) 16px 16px; background: rgba(20, 26, 23, .38); backdrop-filter: blur(3px); }
.command-bar { width: min(680px, 100%); max-height: min(680px, calc(100vh - 32px)); display: grid; grid-template-rows: auto auto minmax(0, 1fr) auto; overflow: hidden; border: 1px solid #9faaa3; border-radius: 8px; background: var(--surface-strong); box-shadow: 0 28px 80px rgba(21, 29, 25, .28); }
.command-bar > header { display: flex; align-items: center; justify-content: space-between; padding: 10px 12px; border-bottom: 1px solid var(--line); }
.command-bar > header > span { display: flex; align-items: center; gap: 7px; color: var(--green); }
.command-bar > header strong { color: var(--ink); font-size: var(--text-2xs); }
.command-input { min-height: 54px; display: grid; grid-template-columns: 22px minmax(0, 1fr); gap: 8px; align-items: center; padding: 0 15px; border-bottom: 1px solid var(--line); color: var(--muted); }
.command-input input { width: 100%; height: 52px; border: 0; outline: 0; background: transparent; color: var(--ink); font: var(--text-md) var(--mono); }
.command-body { min-height: 110px; overflow: auto; }
.command-suggestions { display: grid; padding: 6px; }
.command-suggestions button { min-height: 48px; display: grid; grid-template-columns: minmax(0, 1fr) minmax(120px, auto); gap: 12px; align-items: center; padding: 7px 9px; border: 0; border-radius: 5px; background: transparent; color: var(--ink); text-align: left; }
.command-suggestions button[aria-selected='true'] { background: var(--green-soft); }
.command-suggestions button > span { min-width: 0; display: grid; gap: 2px; }
.command-suggestions strong { font-size: var(--text-xs); }
.command-suggestions small { color: var(--muted); font-size: var(--text-2xs); }
.command-suggestions code { overflow: hidden; color: #53605a; font: var(--text-2xs) var(--mono); text-overflow: ellipsis; white-space: nowrap; }
.command-preview { margin: 8px 12px; padding: 10px; border: 1px solid #a8bfb2; border-radius: 6px; background: #f3f8f4; }
.preview-label { display: block; margin-bottom: 8px; color: var(--green); font-size: var(--text-2xs); font-weight: 800; text-transform: uppercase; }
.command-preview dl { grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; }
.command-preview dl div { display: grid; grid-template-columns: 1fr; gap: 2px; }
.command-preview dt { font-size: var(--text-2xs); }
.command-preview dd { color: var(--ink); font-size: var(--text-xs); font-weight: 700; }
.command-diagnostic { margin: 8px 12px 12px; color: var(--muted); font-size: var(--text-xs); }
.command-diagnostic[data-severity='error'] { color: #9a3e31; }
.command-bar > footer { display: flex; justify-content: flex-end; gap: 8px; padding: 10px 12px; border-top: 1px solid var(--line); background: #f7f9f5; }

@media (max-width: 760px) {
  .command-backdrop { padding: 10px; }
  .command-bar { max-height: calc(100svh - 20px); }
  .command-suggestions button { grid-template-columns: 1fr; gap: 3px; }
  .command-preview dl { grid-template-columns: 1fr; }
}
</style>