<script setup lang="ts">
import { useQueryClient } from '@tanstack/vue-query'
import { computed, ref } from 'vue'

import { ApiError, api } from '../api/client'
import type { Imported } from '../api/transport'

const props = defineProps<{ design: string | null }>()
const emit = defineEmits<{ imported: [design: string] }>()

const client = useQueryClient()

/**
 * The archive waiting on an answer, which knows how to send itself again.
 *
 * Asking somebody to find the file a second time in order to answer a question
 * about it would be a strange thing to do to them, and what has to be held on
 * to differs between a browser and a window.
 */
const conflict = ref<Extract<Imported, { status: 'conflict' }> | null>(null)
const failure = ref<{ message: string; advice: string[] } | null>(null)
const busy = ref(false)

const asking = computed({
  get: () => conflict.value !== null,
  set: (open: boolean) => {
    if (!open) conflict.value = null
  },
})

function exportDesign() {
  if (props.design) void attempt(() => api.exportDesign(props.design as string).then(() => null))
}

function importDesign() {
  void attempt(() => api.importDesign())
}

function replace() {
  const pending = conflict.value
  if (pending) void attempt(() => pending.replace())
}

/**
 * Runs one transfer and reports whatever it turns out to be.
 *
 * Nothing at all means the person changed their mind, which is not something to
 * tell them about.
 */
async function attempt(action: () => Promise<Imported | null>) {
  failure.value = null
  busy.value = true
  try {
    const result = await action()
    if (!result) return
    if (result.status === 'conflict') {
      conflict.value = result
      return
    }
    conflict.value = null
    await client.invalidateQueries({ queryKey: ['designs'] })
    emit('imported', result.design)
  } catch (error) {
    conflict.value = null
    failure.value = {
      message: (error as Error).message,
      advice: error instanceof ApiError ? error.advice : [],
    }
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <!--
    Beside the solver settings rather than inside a menu, because sharing a
    design is something somebody does while looking at it, and a design that
    cannot leave the machine it was written on is one nobody else reviews.
  -->
  <el-tooltip content="Save this design as a .zip" placement="bottom">
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
    <button
      class="action"
      aria-label="Import design"
      data-test="import-design"
      @click="importDesign"
    >
      <el-icon :size="15"><i-upload /></el-icon>
    </button>
  </el-tooltip>

  <!--
    Only ever the one question worth asking. Replacing a design loses whatever
    it held, so it is something a person says rather than something a file name
    decides on their behalf.
  -->
  <el-dialog v-model="asking" title="Replace this design?" width="440px">
    <p class="body" data-test="import-conflict">
      A design called <strong>{{ conflict?.design }}</strong> is already here. Importing over
      it discards everything it holds, and cannot be undone.
    </p>
    <template #footer>
      <el-button size="small" @click="asking = false">Cancel</el-button>
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
      <el-button type="primary" size="small" data-test="import-retry" @click="importDesign">
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
