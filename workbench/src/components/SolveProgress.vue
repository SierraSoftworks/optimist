<script setup lang="ts">
import { computed } from 'vue'

import { useSolveProgress } from '../composables/useSolveProgress'

const props = defineProps<{
  /** Whether a solve is in flight. */
  solving: boolean
  /**
   * What makes two solves cost the same.
   *
   * Everything that decides how much arithmetic there is, and nothing that only
   * decides which answer comes out, so that one variant's timing predicts the
   * next one's.
   */
  shape: string
}>()

const { fraction, caption } = useSolveProgress(
  () => props.solving,
  () => props.shape,
)

const percentage = computed(() => Math.round((fraction.value ?? 0) * 100))
</script>

<template>
  <!--
    Reserved rather than conditional. A bar that appears and disappears moves
    everything under it twice per solve, and this view re-solves on every edit.
  -->
  <div class="solving" :class="{ busy: solving }" data-test="solve-progress">
    <template v-if="solving">
      <el-progress
        :percentage="percentage"
        :indeterminate="fraction === null"
        :duration="2"
        :show-text="false"
        :stroke-width="3"
        aria-label="Solving"
      />
      <span class="caption" aria-live="polite">{{ caption }}</span>
    </template>
  </div>
</template>

<style scoped>
.solving {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: 16px;
  min-width: 152px;
}
.solving :deep(.el-progress) { flex: 1; }
.caption {
  font-size: var(--text-2xs);
  color: var(--muted);
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}
</style>
