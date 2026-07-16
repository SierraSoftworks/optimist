<script setup lang="ts">
import { reactive, ref, watch } from 'vue'
import { Trash2, X } from '@lucide/vue'
import type { GraphEdge, UpdateEdgeInput } from '../api/types'

const props = defineProps<{ open: boolean; pending: boolean; edge: GraphEdge | null }>()
const emit = defineEmits<{
  close: []
  submit: [input: UpdateEdgeInput]
  delete: []
}>()
const form = reactive({ description: '', metadata: '{}' })
const error = ref<string | null>(null)
const confirmDelete = ref(false)

watch(
  () => [props.open, props.edge] as const,
  ([open, edge]) => {
    if (!open || !edge) return
    form.description = edge.description
    form.metadata = JSON.stringify(edge.metadata, null, 2)
    error.value = null
    confirmDelete.value = false
  },
)

function submit() {
  try {
    const metadata = JSON.parse(form.metadata) as unknown
    if (!metadata || Array.isArray(metadata) || typeof metadata !== 'object') {
      throw new Error('Metadata must be a JSON object.')
    }
    emit('submit', {
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
    <div v-if="open && edge" class="dialog-backdrop" @click.self="emit('close')">
      <form class="dialog edge-edit-dialog" aria-labelledby="edit-edge-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">{{ edge.source }} · {{ edge.payload.kind.replaceAll('_', ' ') }} · {{ edge.destination }}</span>
            <h2 id="edit-edge-title">Edit relationship</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <label>
          Description
          <textarea v-model="form.description" rows="6" placeholder="Markdown mechanism or relationship context"></textarea>
        </label>
        <label>
          Metadata
          <textarea v-model="form.metadata" class="code-input" rows="6" spellcheck="false"></textarea>
        </label>
        <p v-if="error" class="form-error">{{ error }}</p>
        <div v-if="confirmDelete" class="replace-warning">
          <Trash2 :size="18" />
          <div><strong>Delete this relationship?</strong><span>The endpoint nodes will remain in the project.</span></div>
        </div>
        <footer>
          <button
            type="button"
            class="danger-button"
            :disabled="pending"
            @click="confirmDelete ? emit('delete') : (confirmDelete = true)"
          ><Trash2 :size="14" /> {{ confirmDelete ? 'Confirm delete' : 'Delete' }}</button>
          <span class="footer-spacer"></span>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending">
            {{ pending ? 'Saving…' : 'Save relationship' }}
          </button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
