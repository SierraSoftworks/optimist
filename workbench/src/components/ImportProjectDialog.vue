<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { AlertTriangle, FileUp, X } from '@lucide/vue'
import type { ProjectArchive } from '../api/types'

const props = defineProps<{
  open: boolean
  pending: boolean
  projectIds: string[]
}>()
const emit = defineEmits<{
  close: []
  submit: [archive: ProjectArchive, replace: boolean]
}>()
const archive = ref<ProjectArchive | null>(null)
const confirmation = ref('')
const error = ref<string | null>(null)
const input = ref<HTMLInputElement>()
const requiresReplace = computed(
  () => archive.value !== null && props.projectIds.includes(archive.value.project.id),
)
const confirmed = computed(
  () => !requiresReplace.value || confirmation.value === archive.value?.project.id,
)

watch(
  () => props.open,
  (open) => {
    if (!open) return
    archive.value = null
    confirmation.value = ''
    error.value = null
    if (input.value) input.value.value = ''
  },
)

async function load(event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0]
  if (!file) return
  try {
    const parsed = JSON.parse(await file.text()) as ProjectArchive
    if (
      parsed.schema_version !== 1 ||
      typeof parsed.project?.id !== 'string' ||
      typeof parsed.project?.name !== 'string' ||
      typeof parsed.files?.['_project.md'] !== 'string' ||
      typeof parsed.summary?.entities !== 'number'
    ) {
      throw new Error('The file is not an Optimist project archive.')
    }
    archive.value = parsed
    error.value = null
  } catch (reason) {
    archive.value = null
    error.value = reason instanceof Error ? reason.message : 'The archive could not be read.'
  }
}

function submit() {
  if (archive.value && confirmed.value) emit('submit', archive.value, requiresReplace.value)
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="dialog-backdrop" @click.self="emit('close')">
      <form class="dialog import-dialog" aria-labelledby="import-project-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">Portable model</span>
            <h2 id="import-project-title">Import project</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>

        <label class="file-picker">
          <FileUp :size="20" />
          <span><strong>Choose archive</strong><small>.optimist.json</small></span>
          <input ref="input" type="file" accept="application/json,.json,.optimist.json" @change="load" />
        </label>
        <p v-if="error" class="form-error">{{ error }}</p>
        <section v-if="archive" class="archive-preview">
          <div><span class="eyebrow">Project</span><strong>{{ archive.project.name }}</strong><code>{{ archive.project.id }} · r{{ archive.project.revision }}</code></div>
          <dl>
            <div><dt>Entities</dt><dd>{{ archive.summary.entities }}</dd></div>
            <div><dt>Relationships</dt><dd>{{ archive.summary.edges }}</dd></div>
            <div><dt>Scenarios</dt><dd>{{ archive.summary.scenarios }}</dd></div>
          </dl>
        </section>
        <div v-if="requiresReplace" class="replace-warning">
          <AlertTriangle :size="18" />
          <div>
            <strong>This replaces project {{ archive?.project.id }}</strong>
            <span>Current graph data and process-local change history for this project will be replaced.</span>
          </div>
        </div>
        <label v-if="requiresReplace">
          Type {{ archive?.project.id }} to confirm
          <input v-model="confirmation" autocomplete="off" :placeholder="archive?.project.id" />
        </label>
        <footer>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending || !archive || !confirmed">
            {{ pending ? 'Importing…' : requiresReplace ? 'Replace project' : 'Import project' }}
          </button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
