<script setup lang="ts">
import type { SaveState } from '../composables/useDraft'

defineProps<{
  state: SaveState
  error?: string | null
  advice?: string[]
}>()

const emit = defineEmits<{ revert: [] }>()
</script>

<template>
  <!--
    Deliberately quiet. A field that is saved is the normal case and does not
    need announcing for long; a field that was refused has to say so until
    somebody deals with it, because the value on screen is not the value stored.
  -->
  <span class="status" :class="state">
    <el-icon v-if="state === 'saving'" class="spin"><i-loading /></el-icon>
    <el-icon v-else-if="state === 'saved'" class="ok"><i-check /></el-icon>
    <el-popover v-else-if="state === 'failed'" trigger="hover" placement="left" :width="320">
      <template #reference>
        <el-icon class="bad" tabindex="0" role="button" aria-label="Show why this was refused">
          <i-warning-filled />
        </el-icon>
      </template>
      <div class="explain">
        <p class="message">{{ error }}</p>
        <ul v-if="advice?.length" class="advice">
          <li v-for="line in advice" :key="line">{{ line }}</li>
        </ul>
        <el-button size="small" text @click="emit('revert')">Discard this change</el-button>
      </div>
    </el-popover>
    <span v-else-if="state === 'editing'" class="pending" aria-label="Unsaved" />
  </span>
</template>

<style scoped>
.status { display: inline-flex; align-items: center; justify-content: center; width: 16px; height: 16px; flex: 0 0 auto; }
.ok { color: #2f9e69; }
.bad { color: var(--danger); cursor: help; }
.pending { width: 5px; height: 5px; border-radius: 50%; background: var(--muted); }
.spin { animation: spin 1s linear infinite; color: var(--muted); }
@keyframes spin { to { transform: rotate(360deg); } }
.explain { display: flex; flex-direction: column; gap: var(--space-2); }
.message { margin: 0; font-size: var(--text-xs); color: var(--danger); }
.advice { margin: 0; padding-left: 1.1em; font-size: var(--text-2xs); color: var(--muted); }
</style>
