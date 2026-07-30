<script setup lang="ts">
import { useQueryClient } from '@tanstack/vue-query'
import { ref } from 'vue'

import { ApiError, api } from '../api/client'

const props = defineProps<{ design: string | null }>()
const emit = defineEmits<{ imported: [design: string] }>()

const client = useQueryClient()
const file = ref<HTMLInputElement | null>(null)

/**
 * The archive waiting on an answer, kept so the same bytes can be sent again.
 *
 * Asking somebody to find the file a second time in order to answer a question
 * about it would be a strange thing to do to them.
 */
const pending = ref<{ id: string; archive: File } | null>(null)
const conflict = ref(false)
const failure = ref<{ message: string; advice: string[] } | null>(null)
const busy = ref(false)

/** A directory name, matching the rule the server enforces. */
function slug(value: string): string {
  return value
    .replace(/\.zip$/i, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '')
    .slice(0, 128)
}

/**
 * Hands the archive to the browser rather than fetching it here.
 *
 * The server already says what the file is called and that it is a download, so
 * it streams to disk rather than being assembled in this tab first.
 */
function exportDesign() {
  if (!props.design) return
  const link = document.createElement('a')
  link.href = api.archiveUrl(props.design)
  link.download = `${props.design}.zip`
  link.click()
}

function choose() {
  failure.value = null
  file.value?.click()
}

async function chosen(event: Event) {
  const input = event.target as HTMLInputElement
  const archive = input.files?.[0]
  // Clearing it means choosing the same file twice still fires a change.
  input.value = ''
  if (!archive) return

  const id = slug(archive.name)
  if (!id) {
    failure.value = {
      message: `'${archive.name}' cannot name a design.`,
      advice: ['Rename the file using letters and digits, then choose it again.'],
    }
    return
  }
  await send({ id, archive }, false)
}

async function send(request: { id: string; archive: File }, replace: boolean) {
  busy.value = true
  try {
    await api.importArchive(request.id, request.archive, replace)
    await client.invalidateQueries({ queryKey: ['designs'] })
    pending.value = null
    conflict.value = false
    emit('imported', request.id)
  } catch (error) {
    if (error instanceof ApiError && error.status === 409) {
      pending.value = request
      conflict.value = true
      return
    }
    pending.value = null
    conflict.value = false
    failure.value = {
      message: (error as Error).message,
      advice: error instanceof ApiError ? error.advice : [],
    }
  } finally {
    busy.value = false
  }
}

function replace() {
  if (pending.value) void send(pending.value, true)
}
</script>

<template>
  <!--
    Beside the solver settings rather than inside a menu, because sharing a
    design is something somebody does while looking at it, and a design that
    cannot leave the machine it was written on is one nobody else reviews.
  -->
  <el-tooltip content="Download this design as a .zip" placement="bottom">
    <button
      class="action"
      aria-label="Export design"
      data-test="export-design"
      :disabled="!design"
      @click="exportDesign"
    >
      <el-icon :size="15"><i-download /></el-icon>
    </button>
  </el-tooltip>

  <el-tooltip content="Import a design from a .zip" placement="bottom">
    <button class="action" aria-label="Import design" data-test="import-design" @click="choose">
      <el-icon :size="15"><i-upload /></el-icon>
    </button>
  </el-tooltip>

  <input
    ref="file"
    type="file"
    accept=".zip,application/zip"
    class="hidden"
    data-test="import-file"
    @change="chosen"
  />

  <!--
    Only ever the one question worth asking. Replacing a design loses whatever
    it held, so it is something a person says rather than something a file name
    decides on their behalf.
  -->
  <el-dialog v-model="conflict" title="Replace this design?" width="440px">
    <p class="body" data-test="import-conflict">
      A design called <strong>{{ pending?.id }}</strong> is already here. Importing over it
      discards everything it holds, and cannot be undone.
    </p>
    <template #footer>
      <el-button size="small" @click="conflict = false">Cancel</el-button>
      <el-button
        type="danger"
        size="small"
        :loading="busy"
        data-test="import-replace"
        @click="replace"
      >
        Replace
      </el-button>
    </template>
  </el-dialog>

  <!--
    The server explains what to do about a rejected archive as well as what was
    wrong with it, and an archive is refused for reasons the person holding it
    can usually act on: it arrived truncated, it was zipped by hand, or it came
    from a build that writes a schema this one does not read.
  -->
  <el-dialog
    :model-value="failure !== null"
    title="This archive was not imported"
    width="480px"
    @update:model-value="failure = null"
  >
    <el-alert type="error" :closable="false" show-icon :title="failure?.message ?? ''" />
    <ul v-if="failure?.advice.length" class="advice" data-test="import-advice">
      <li v-for="line in failure.advice" :key="line">{{ line }}</li>
    </ul>
    <template #footer>
      <el-button size="small" @click="failure = null">Close</el-button>
      <el-button type="primary" size="small" data-test="import-retry" @click="choose">
        Choose another file
      </el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--muted);
}
.action:hover:not(:disabled) { background: var(--green-soft); color: var(--green); }
.action:disabled { opacity: 0.4; }
.hidden { display: none; }
.body { margin: 0; font-size: var(--text-sm); line-height: 1.5; }
.advice {
  margin: var(--space-3) 0 0;
  padding-left: var(--space-4);
  font-size: var(--text-xs);
  color: var(--muted);
  line-height: 1.6;
}
</style>
