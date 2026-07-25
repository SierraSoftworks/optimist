<script setup lang="ts">
import { Check, ChevronDown, Plus } from '@lucide/vue'
import { computed, nextTick, onBeforeUnmount, ref } from 'vue'
import type { Scenario } from '../api/types'

const props = defineProps<{
  scenarios: Scenario[]
  selectedScenarioId: string | null
}>()
const emit = defineEmits<{
  select: [id: string]
  create: []
}>()
const open = ref(false)
const trigger = ref<HTMLButtonElement>()
const menu = ref<HTMLElement>()
const optionButtons = ref<Array<HTMLButtonElement>>([])
const position = ref({ top: '0px', left: '0px', width: '260px', maxHeight: '320px' })
const selected = computed(() =>
  props.scenarios.find((scenario) => scenario.id === props.selectedScenarioId) ?? props.scenarios[0],
)

function place() {
  if (!trigger.value) return
  const anchor = trigger.value.getBoundingClientRect()
  const panelHeight = menu.value?.scrollHeight ?? 0
  const margin = 12
  const gap = 5
  const width = Math.min(320, Math.max(240, anchor.width), window.innerWidth - margin * 2)
  const left = Math.min(
    Math.max(margin, anchor.left),
    window.innerWidth - margin - width,
  )
  const below = Math.max(0, window.innerHeight - margin - anchor.bottom - gap)
  const above = Math.max(0, anchor.top - margin - gap)
  const openAbove = panelHeight > below && above > below
  const maxHeight = Math.max(120, openAbove ? above : below)
  const top = openAbove
    ? Math.max(margin, anchor.top - gap - Math.min(panelHeight, maxHeight))
    : anchor.bottom + gap
  position.value = {
    top: `${top}px`,
    left: `${left}px`,
    width: `${width}px`,
    maxHeight: `${maxHeight}px`,
  }
}

function listen() {
  window.addEventListener('resize', place)
  window.addEventListener('scroll', place, true)
  document.addEventListener('pointerdown', outside)
  document.addEventListener('keydown', keys)
}

function unlisten() {
  window.removeEventListener('resize', place)
  window.removeEventListener('scroll', place, true)
  document.removeEventListener('pointerdown', outside)
  document.removeEventListener('keydown', keys)
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
  optionButtons.value.find((button) => button.dataset.selected === 'true')?.focus()
}

function close(focus = false) {
  open.value = false
  unlisten()
  if (focus) trigger.value?.focus()
}

function choose(id: string) {
  emit('select', id)
  close(true)
}

function create() {
  emit('create')
  close(false)
}

function outside(event: PointerEvent) {
  const target = event.target as Node
  if (!menu.value?.contains(target) && !trigger.value?.contains(target)) close()
}

function keys(event: KeyboardEvent) {
  if (!open.value) return
  if (event.key === 'Escape') {
    event.preventDefault()
    close(true)
    return
  }
  if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return
  const buttons = optionButtons.value
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

onBeforeUnmount(unlisten)
</script>

<template>
  <div class="scenario-picker">
    <span class="section-label">Scenario</span>
    <button ref="trigger" type="button" class="scenario-picker-trigger" aria-haspopup="listbox" :aria-expanded="open" @click="toggle">
      <span><strong>{{ selected?.title }}</strong><small v-if="selected">{{ selected.id }} · r{{ selected.revision }} · {{ selected.planning_horizon }} periods</small></span>
      <ChevronDown :size="15" />
    </button>
    <Teleport to="body">
      <div v-if="open" ref="menu" class="scenario-menu" :style="position" role="listbox" aria-label="Scenarios">
        <button
          v-for="(scenario, index) in scenarios"
          :key="scenario.id"
          :ref="(element) => { if (element) optionButtons[index] = element as HTMLButtonElement }"
          type="button"
          role="option"
          :aria-selected="scenario.id === selectedScenarioId"
          :data-selected="scenario.id === selectedScenarioId"
          @click="choose(scenario.id)"
        >
          <Check :size="14" :class="{ hidden: scenario.id !== selectedScenarioId }" />
          <span><strong>{{ scenario.title }}</strong><small>{{ scenario.id }} · r{{ scenario.revision }} · {{ scenario.objectives.length }} objective{{ scenario.objectives.length === 1 ? '' : 's' }}</small></span>
        </button>
        <button type="button" class="scenario-menu-create" @click="create"><Plus :size="14" /><span>New scenario</span></button>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.scenario-picker { min-width: 0; display: grid; gap: 5px; }
.scenario-picker-trigger { width: 100%; min-height: 42px; display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; gap: 8px; padding: 6px 9px; border: 1px solid var(--line); border-radius: 5px; background: white; color: var(--ink); text-align: left; }
.scenario-picker-trigger:hover, .scenario-picker-trigger[aria-expanded='true'] { border-color: #95a39a; background: #f7f9f5; }
.scenario-picker-trigger > span { min-width: 0; display: grid; gap: 2px; }
.scenario-picker-trigger strong { overflow: hidden; text-overflow: ellipsis; font-size: var(--text-xs); white-space: nowrap; }
.scenario-picker-trigger small { color: var(--muted); font: var(--text-2xs) var(--mono); }
.scenario-menu { position: fixed; z-index: 80; overflow: auto; border: 1px solid #aeb9b1; border-radius: 6px; background: white; box-shadow: 0 12px 32px rgba(30, 40, 34, .18); }
.scenario-menu > button { width: 100%; display: grid; grid-template-columns: 18px minmax(0, 1fr); gap: 7px; align-items: center; padding: 8px 9px; border: 0; background: transparent; color: var(--ink); text-align: left; }
.scenario-menu > button:hover, .scenario-menu > button:focus-visible, .scenario-menu > button[aria-selected='true'] { background: var(--green-soft); }
.scenario-menu > button > span { min-width: 0; display: grid; gap: 2px; }
.scenario-menu strong { overflow: hidden; text-overflow: ellipsis; font-size: var(--text-xs); white-space: nowrap; }
.scenario-menu small { color: var(--muted); font: var(--text-2xs) var(--mono); }
.scenario-menu svg.hidden { opacity: 0; }
.scenario-menu .scenario-menu-create { border-top: 1px solid var(--line); color: var(--green); font-size: var(--text-xs); font-weight: 700; }
</style>
