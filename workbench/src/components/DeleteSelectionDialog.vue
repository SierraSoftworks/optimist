<script setup lang="ts">
import { Trash2, X } from '@lucide/vue'

withDefaults(defineProps<{
  open: boolean
  pending: boolean
  kind: 'node' | 'relationship'
  title: string
  blockedReason?: string | null
}>(), { blockedReason: null })
const emit = defineEmits<{ close: []; confirm: [] }>()
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="dialog-backdrop" @pointerdown.self="emit('close')">
      <form class="dialog delete-selection-dialog" aria-labelledby="delete-selection-title" @submit.prevent="emit('confirm')">
        <header>
          <div><span class="eyebrow">Graph selection</span><h2 id="delete-selection-title">Delete {{ kind }}</h2></div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <div class="delete-selection-summary"><Trash2 :size="20" /><div><strong>{{ title }}</strong><span v-if="blockedReason">{{ blockedReason }}</span><span v-else>This cannot be undone. The rest of the model remains unchanged.</span></div></div>
        <footer>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="danger-button" :disabled="pending || Boolean(blockedReason)"><Trash2 :size="14" /> {{ pending ? 'Deleting…' : `Delete ${kind}` }}</button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>

<style scoped>
.delete-selection-summary { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 10px; align-items: start; padding: 12px; border: 1px solid #d8a098; border-radius: 5px; background: #fff8f6; color: #8c3429; }
.delete-selection-summary div { display: grid; gap: 3px; }
.delete-selection-summary strong { color: var(--ink); font-size: 11px; }
.delete-selection-summary span { font-size: 9px; line-height: 1.45; }
</style>
