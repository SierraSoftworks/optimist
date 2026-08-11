<script setup lang="ts">
import { useQueryClient, useQuery } from '@tanstack/vue-query'
import { computed, ref } from 'vue'

import { ApiError, api } from '../api/client'

const emit = defineEmits<{ changed: [] }>()

const client = useQueryClient()
const folder = api.workspace

const { data: current } = useQuery({
  queryKey: ['workspace'],
  queryFn: () => folder?.current() ?? '',
  enabled: !!folder,
})

/**
 * The last part of the path, which is what tells one folder from another.
 *
 * The whole path is in the tooltip: it is what somebody needs when they are
 * checking, and noise the rest of the time.
 */
const name = computed(() => current.value?.split(/[\\/]/).filter(Boolean).pop() ?? '')

const busy = ref(false)
const failure = ref<string | null>(null)

async function choose() {
  if (!folder || busy.value) return
  busy.value = true
  try {
    const chosen = await folder.choose()
    if (!chosen) return
    // Everything held describes designs in the folder that was open, and none
    // of it is true of the one that is.
    await client.invalidateQueries()
    emit('changed')
  } catch (error) {
    failure.value = error instanceof ApiError ? error.message : String(error)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <el-tooltip v-if="folder" :content="current ?? ''" placement="bottom">
    <button class="folder" data-test="workspace-folder" :disabled="busy" @click="choose">
      <el-icon :size="14"><i-folder-opened /></el-icon>
      <span>{{ name }}</span>
    </button>
  </el-tooltip>

  <el-dialog
    :model-value="failure !== null"
    title="That folder was not opened"
    width="440px"
    @update:model-value="failure = null"
  >
    <el-alert type="error" :closable="false" show-icon :title="failure ?? ''" />
    <template #footer>
      <el-button size="small" @click="failure = null">Close</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.folder {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  height: 26px;
  padding: 0 0.55rem;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  background: none;
  color: var(--el-text-color-regular);
  font: inherit;
  font-size: 0.78rem;
  cursor: pointer;
}

.folder:hover:not(:disabled) {
  border-color: var(--el-color-primary);
  color: var(--el-color-primary);
}

.folder:disabled {
  cursor: progress;
  opacity: 0.6;
}
</style>
