<script setup lang="ts">
import { ref } from 'vue'
import { CircleHelp, X } from '@lucide/vue'

const props = defineProps<{ label: string; text: string }>()
const open = ref(false)
</script>

<template>
  <span class="parameter-help">
    <button
      type="button"
      class="parameter-help-trigger"
      :aria-label="`Explain ${label}`"
      :aria-expanded="open"
      @click="open = !open"
    ><CircleHelp :size="14" /></button>
    <span v-if="open" class="parameter-popover" role="note">
      <strong>{{ label }}</strong>
      <span>{{ text }}</span>
      <button type="button" aria-label="Close explanation" @click="open = false"><X :size="12" /></button>
    </span>
  </span>
</template>
