<script setup lang="ts">
import { reactive, ref, watch } from 'vue'
import { Pencil, Trash2, X } from '@lucide/vue'
import type { Distribution, EdgeEstimateSlot, GraphEdge, UpdateEdgeInput } from '../api/types'

const props = defineProps<{ open: boolean; pending: boolean; edge: GraphEdge | null }>()
const emit = defineEmits<{
  close: []
  submit: [input: UpdateEdgeInput]
  delete: []
  estimate: [slot: EdgeEstimateSlot]
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

function distributionLabel(value: Distribution) {
  if (value.type === 'point') return `Point · ${value.value}`
  if (value.type === 'beta') return `Beta · α ${value.alpha}, β ${value.beta}`
  if (value.type === 'scaled_beta') return `Scaled Beta · [${value.lower}, ${value.upper}]`
  if (value.type === 'normal') return `Normal · μ ${value.mean}, σ ${value.standard_deviation}`
  return `LogNormal · μ ${value.location}, σ ${value.scale}`
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
        <section v-if="edge.payload.kind === 'contributes' || edge.payload.kind === 'changes'" class="dialog-section">
          <div class="estimate-row">
            <div><span>Effect</span><strong>{{ distributionLabel(edge.payload.properties.effect.distribution) }}</strong></div>
            <button type="button" class="icon-button" aria-label="Edit relationship effect estimate" @click="emit('estimate', { kind: 'effect' })"><Pencil :size="13" /></button>
          </div>
          <div class="estimate-row">
            <div><span>Lag</span><strong>{{ edge.payload.properties.lag ? distributionLabel(edge.payload.properties.lag.distribution) : 'Not set' }}</strong></div>
            <button type="button" class="icon-button" aria-label="Edit relationship lag estimate" @click="emit('estimate', { kind: 'lag' })"><Pencil :size="13" /></button>
          </div>
          <dl class="relationship-context">
            <div><dt>Mechanism</dt><dd>{{ edge.payload.properties.mechanism || 'Not documented' }}</dd></div>
            <div><dt>Evidence</dt><dd>{{ edge.payload.properties.evidence.join('; ') || 'None' }}</dd></div>
          </dl>
        </section>
        <section v-else-if="edge.payload.kind === 'blocks'" class="dialog-section">
          <div class="estimate-row">
            <div><span>Blocking degree</span><strong>{{ distributionLabel(edge.payload.properties.degree.distribution) }}</strong></div>
            <button type="button" class="icon-button" aria-label="Edit blocking degree estimate" @click="emit('estimate', { kind: 'degree' })"><Pencil :size="13" /></button>
          </div>
        </section>
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
