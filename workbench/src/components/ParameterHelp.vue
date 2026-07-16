<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref } from 'vue'
import { CircleHelp, X } from '@lucide/vue'

const props = defineProps<{ label: string; text: string }>()
const open = ref(false)
const trigger = ref<HTMLButtonElement>()
const popover = ref<HTMLElement>()
const position = ref({ top: '0px', left: '0px', maxWidth: '280px' })

function place() {
  if (!trigger.value || !popover.value) return
  const anchor = trigger.value.getBoundingClientRect()
  const panel = popover.value.getBoundingClientRect()
  const gap = 6
  const margin = 12
  const availableWidth = Math.max(180, window.innerWidth - margin * 2)
  const width = Math.min(280, availableWidth)
  let left = anchor.left
  if (left + width > window.innerWidth - margin) left = window.innerWidth - margin - width
  left = Math.max(margin, left)
  const below = anchor.bottom + gap
  const top = below + panel.height <= window.innerHeight - margin
    ? below
    : Math.max(margin, anchor.top - gap - panel.height)
  position.value = { top: `${top}px`, left: `${left}px`, maxWidth: `${width}px` }
}

function listen() {
  window.addEventListener('resize', place)
  window.addEventListener('scroll', place, true)
}

function unlisten() {
  window.removeEventListener('resize', place)
  window.removeEventListener('scroll', place, true)
}

async function toggle() {
  open.value = !open.value
  if (!open.value) {
    unlisten()
    return
  }
  await nextTick()
  place()
  listen()
}

function close() {
  open.value = false
  unlisten()
  trigger.value?.focus()
}

onBeforeUnmount(unlisten)
</script>

<template>
  <span class="parameter-help">
    <button
      ref="trigger"
      type="button"
      class="parameter-help-trigger"
      :aria-label="`Explain ${label}`"
      :aria-expanded="open"
      @click="toggle"
    ><CircleHelp :size="14" /></button>
    <Teleport to="body">
      <span v-if="open" ref="popover" class="parameter-popover" role="note" :style="position">
        <strong>{{ label }}</strong>
        <span>{{ text }}</span>
        <button type="button" aria-label="Close explanation" @click="close"><X :size="12" /></button>
      </span>
    </Teleport>
  </span>
</template>
