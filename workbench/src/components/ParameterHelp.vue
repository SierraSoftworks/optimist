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

<style scoped>
.parameter-help { position: relative; display: inline-flex; font-weight: 400; }
.parameter-help-trigger { width: 20px; height: 20px; display: grid; place-items: center; padding: 0; border: 0; border-radius: 4px; background: transparent; color: var(--muted); }
.parameter-help-trigger:hover, .parameter-help-trigger[aria-expanded='true'] { background: var(--green-soft); color: var(--green); }
.parameter-help-trigger:focus-visible { outline: 2px solid #2a7059; outline-offset: 1px; }
.parameter-popover { position: fixed; z-index: 80; width: 280px; display: grid; gap: 5px; padding: 10px 34px 10px 11px; border: 1px solid #aeb9b1; border-radius: 6px; background: white; color: var(--ink); box-shadow: 0 10px 28px rgba(30, 40, 34, .16); font-size: 10px; font-weight: 400; line-height: 1.5; }
.parameter-popover strong { font-size: 10px; }
.parameter-popover > span { color: #55605a; }
.parameter-popover > button { position: absolute; top: 5px; right: 5px; width: 23px; height: 23px; display: grid; place-items: center; padding: 0; border: 0; border-radius: 4px; background: transparent; color: var(--muted); }
.parameter-popover > button:hover { background: #edf0eb; }

@media (max-width: 760px) {
  .parameter-popover { width: min(280px, calc(100vw - 24px)); }
}
</style>
