<script setup lang="ts">
import { reactive, ref, watch } from 'vue'
import { Trash2, X } from '@lucide/vue'
import type { Evidence, EvidenceInput, GraphNode } from '../api/types'

const props = defineProps<{
  open: boolean
  pending: boolean
  node: GraphNode | null
  evidence: Evidence | null
}>()
const emit = defineEmits<{
  close: []
  submit: [input: EvidenceInput]
  delete: []
}>()
const form = reactive({ summary: '', source: '' })
const confirmDelete = ref(false)

watch(
  () => [props.open, props.evidence] as const,
  ([open, evidence]) => {
    if (!open) return
    form.summary = evidence?.summary ?? ''
    form.source = evidence?.source ?? ''
    confirmDelete.value = false
  },
)

function submit() {
  const summary = form.summary.trim()
  if (!summary) return
  emit('submit', { summary, source: form.source.trim() || null })
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open && node" class="dialog-backdrop" @click.self="emit('close')">
      <form class="dialog" aria-labelledby="edit-evidence-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">{{ node.title }}<template v-if="evidence"> · evidence #{{ evidence.id }} · r{{ evidence.revision }}</template></span>
            <h2 id="edit-evidence-title">{{ evidence ? 'Edit evidence' : 'Add evidence' }}</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <label>Summary<textarea v-model="form.summary" rows="5" placeholder="Concise observation or symptom" required></textarea></label>
        <label>Source<input v-model="form.source" placeholder="Citation, URL, system, or person" /></label>
        <div v-if="confirmDelete" class="replace-warning">
          <Trash2 :size="18" />
          <div><strong>Delete this evidence?</strong><span>This removes the record from its owning node.</span></div>
        </div>
        <footer>
          <button v-if="evidence" type="button" class="danger-button" :disabled="pending" @click="confirmDelete ? emit('delete') : (confirmDelete = true)"><Trash2 :size="14" /> {{ confirmDelete ? 'Confirm delete' : 'Delete' }}</button>
          <span class="footer-spacer"></span>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending || !form.summary.trim()">{{ pending ? 'Saving…' : evidence ? 'Save evidence' : 'Add evidence' }}</button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
