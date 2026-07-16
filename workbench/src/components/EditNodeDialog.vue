<script setup lang="ts">
import { reactive, ref, watch } from 'vue'
import { X } from '@lucide/vue'
import type { GraphNode, UpdateNodeInput } from '../api/types'

const props = defineProps<{ open: boolean; pending: boolean; node: GraphNode | null }>()
const emit = defineEmits<{ close: []; submit: [input: UpdateNodeInput] }>()
const form = reactive({ title: '', description: '', metadata: '{}' })
const error = ref<string | null>(null)

watch(
  () => [props.open, props.node] as const,
  ([open, node]) => {
    if (!open || !node) return
    form.title = node.title
    form.description = node.description
    form.metadata = JSON.stringify(node.metadata, null, 2)
    error.value = null
  },
)

function submit() {
  if (!form.title.trim()) return
  try {
    const metadata = JSON.parse(form.metadata) as unknown
    if (!metadata || Array.isArray(metadata) || typeof metadata !== 'object') {
      throw new Error('Metadata must be a JSON object.')
    }
    emit('submit', {
      title: form.title.trim(),
      description: form.description,
      metadata: metadata as Record<string, unknown>,
    })
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : 'Metadata is invalid JSON.'
  }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open && node" class="dialog-backdrop" @click.self="emit('close')">
      <form class="dialog node-edit-dialog" aria-labelledby="edit-node-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">{{ node.payload.kind }} · {{ node.id }}</span>
            <h2 id="edit-node-title">Edit node details</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <label>
          Title
          <input v-model="form.title" required />
        </label>
        <label>
          Description
          <textarea v-model="form.description" rows="6" placeholder="Markdown description and model boundaries"></textarea>
        </label>
        <label>
          Metadata
          <textarea v-model="form.metadata" class="code-input" rows="6" spellcheck="false"></textarea>
        </label>
        <p v-if="error" class="form-error">{{ error }}</p>
        <footer>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending || !form.title.trim()">
            {{ pending ? 'Saving…' : 'Save details' }}
          </button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
