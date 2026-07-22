<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { Link } from '@lucide/vue'
import type { EdgeKind, GraphNode } from '../api/types'
import { destinationsFor, edgeKinds } from '../domain/edgeAuthoring'

const props = defineProps<{
  open: boolean
  source: GraphNode | null
  nodes: GraphNode[]
  x: number
  y: number
}>()
const emit = defineEmits<{
  close: []
  select: [kind: EdgeKind]
}>()
const menu = ref<HTMLElement>()
const position = ref({ top: '0px', left: '0px' })
const availableKinds = computed(() =>
  edgeKinds.filter(({ kind }) => destinationsFor(kind, props.source ?? undefined, props.nodes).length > 0),
)

function place() {
  if (!menu.value) return
  const margin = 12
  const bounds = menu.value.getBoundingClientRect()
  position.value = {
    top: `${Math.min(Math.max(margin, props.y), window.innerHeight - margin - bounds.height)}px`,
    left: `${Math.min(Math.max(margin, props.x), window.innerWidth - margin - bounds.width)}px`,
  }
}

function close() {
  emit('close')
}

function select(kind: EdgeKind) {
  emit('select', kind)
}

function outside(event: PointerEvent) {
  if (!menu.value?.contains(event.target as Node)) close()
}

function keys(event: KeyboardEvent) {
  if (!props.open) return
  if (event.key === 'Escape') {
    event.preventDefault()
    close()
    return
  }
  if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return
  const buttons = Array.from(menu.value?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]') ?? [])
  if (!buttons.length) return
  event.preventDefault()
  const current = Math.max(0, buttons.indexOf(document.activeElement as HTMLButtonElement))
  const next = event.key === 'Home'
    ? 0
    : event.key === 'End'
      ? buttons.length - 1
      : event.key === 'ArrowDown'
        ? (current + 1) % buttons.length
        : (current - 1 + buttons.length) % buttons.length
  buttons[next]?.focus()
}

function unlisten() {
  document.removeEventListener('pointerdown', outside)
  document.removeEventListener('keydown', keys)
  window.removeEventListener('resize', close)
  window.removeEventListener('scroll', close, true)
}

watch(() => [props.open, props.source?.id, props.x, props.y], async ([open]) => {
  unlisten()
  if (!open) return
  await nextTick()
  place()
  menu.value?.querySelector<HTMLButtonElement>('[role="menuitem"]')?.focus()
  document.addEventListener('pointerdown', outside)
  document.addEventListener('keydown', keys)
  window.addEventListener('resize', close)
  window.addEventListener('scroll', close, true)
})

onBeforeUnmount(unlisten)
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open && source"
      ref="menu"
      class="node-relationship-menu"
      :style="position"
      role="menu"
      :aria-label="`Add relationship from ${source.title}`"
    >
      <div class="node-relationship-menu-heading">
        <Link :size="14" />
        <span><small>Add relationship from</small><strong>{{ source.title }}</strong></span>
      </div>
      <button
        v-for="item in availableKinds"
        :key="item.kind"
        type="button"
        role="menuitem"
        @click="select(item.kind)"
      >
        {{ item.label }}
      </button>
    </div>
  </Teleport>
</template>

<style scoped>
.node-relationship-menu { position: fixed; z-index: 80; min-width: 190px; overflow: hidden; border: 1px solid #aeb9b1; border-radius: 6px; background: white; box-shadow: 0 12px 32px rgba(30, 40, 34, .18); }
.node-relationship-menu-heading { display: grid; grid-template-columns: 18px minmax(0, 1fr); gap: 7px; align-items: center; padding: 9px; border-bottom: 1px solid var(--line); color: var(--green); }
.node-relationship-menu-heading > span { min-width: 0; display: grid; gap: 1px; }
.node-relationship-menu-heading small { color: var(--muted); font-size: 8px; }
.node-relationship-menu-heading strong { max-width: 220px; overflow: hidden; color: var(--ink); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
.node-relationship-menu > button { width: 100%; padding: 7px 10px 7px 34px; border: 0; background: transparent; color: var(--ink); font-size: 10px; text-align: left; }
.node-relationship-menu > button:hover, .node-relationship-menu > button:focus-visible { background: var(--green-soft); }
</style>