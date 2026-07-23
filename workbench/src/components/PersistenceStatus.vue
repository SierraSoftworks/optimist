<script setup lang="ts">
import { AlertTriangle, RefreshCw } from '@lucide/vue'
import { useServerHealth } from '../composables/useProjectData'

const health = useServerHealth()
</script>

<template>
  <span
    v-if="health.data.value?.persistence.state && health.data.value.persistence.state !== 'idle'"
    class="persistence-state"
    :data-state="health.data.value.persistence.state"
    :title="health.data.value.persistence.error ?? 'Compacting durable model snapshot'"
    aria-live="polite"
  >
    <RefreshCw v-if="health.data.value.persistence.state === 'pending'" class="spin" :size="13" />
    <AlertTriangle v-else :size="13" />
    {{ health.data.value.persistence.state === 'pending' ? 'Saving model' : 'Persistence degraded' }}
  </span>
</template>

<style scoped>
.persistence-state { display: inline-flex; align-items: center; gap: 5px; color: var(--muted); font-size: 8px; font-weight: 700; white-space: nowrap; }
.persistence-state[data-state='error'] { color: #9a3e31; }
</style>