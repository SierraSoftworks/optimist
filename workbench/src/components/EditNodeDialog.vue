<script setup lang="ts">
import { ref, watch } from 'vue'
import { X } from '@lucide/vue'
import type { GraphNode, UpdateNodeInput } from '../api/types'

const props = defineProps<{ open: boolean; pending: boolean; node: GraphNode | null }>()
const emit = defineEmits<{ close: []; submit: [input: UpdateNodeInput] }>()
const title = ref('')

watch(
  () => [props.open, props.node] as const,
  ([open, node]) => {
    if (!open || !node) return
    title.value = node.title
  },
)

function submit() {
  if (!title.value.trim() || !props.node) return
  emit('submit', {
    title: title.value.trim(),
    description: props.node.description,
    metadata: props.node.metadata,
  })
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open && node" class="dialog-backdrop" @pointerdown.self="emit('close')">
      <form class="dialog node-edit-dialog" aria-labelledby="edit-node-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">{{ node.payload.kind }} · {{ node.id }}</span>
            <h2 id="edit-node-title">Edit node</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <label>
          Title
          <input v-model="title" required />
        </label>
        <footer>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending || !title.trim()">
            {{ pending ? 'Saving…' : 'Save' }}
          </button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
