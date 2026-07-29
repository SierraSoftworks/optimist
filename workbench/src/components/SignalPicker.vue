<script setup lang="ts">
import { computed, ref } from 'vue'

import { describeChange, directionOf } from '../domain/change'

/** One quantity that can be watched, and where it came from. */
export interface SignalOption {
  value: string
  component: string
  channel: string
  family: 'input' | 'backpressure' | 'channel'
}

const props = defineProps<{
  options: SignalOption[]
  pinned: string[]
  /**
   * What each quantity settled at with and without the variant.
   *
   * Empty while the design itself is on show, because there is nothing to have
   * moved from. Given for every quantity rather than only the pinned ones, so
   * that finding what a proposal changed is a matter of reading the list rather
   * than charting things one at a time until something moves.
   */
  moved?: Record<string, { before: number; after: number }>
}>()
const emit = defineEmits<{ 'update:pinned': [string[]] }>()

const search = ref('')

function movement(value: string) {
  const pair = props.moved?.[value]
  if (!pair) return null
  const label = describeChange(pair.before, pair.after)
  const direction = directionOf(pair.before, pair.after)
  return label && direction ? { label, direction } : null
}

/** How far a quantity moved, for ordering by it. */
function magnitude(value: string): number {
  const pair = props.moved?.[value]
  if (!pair || pair.before === 0) return pair ? Number.MAX_SAFE_INTEGER : 0
  return Math.abs((pair.after - pair.before) / Math.abs(pair.before))
}

/**
 * How a quantity reached its component, in the order a reader works through it.
 *
 * The component's own channels first, because that is what somebody came to
 * look at; what arrived and what came back are context for it.
 */
const FAMILIES = [
  { key: 'channel', label: 'Solved quantities' },
  { key: 'input', label: 'Arriving demand' },
  { key: 'backpressure', label: 'Returning from dependencies' },
] as const

const byValue = computed(() => new Map(props.options.map((option) => [option.value, option])))

/**
 * The pinned quantities, in the order they were pinned.
 *
 * Ordered by the list rather than re-sorted, so the chart a reader added last is
 * where they last looked rather than somewhere alphabetical.
 */
const chosen = computed(() =>
  props.pinned.map((value) => byValue.value.get(value) ?? null).filter((option) => !!option),
)

/**
 * Alphabetical by the quantity's own name, then by the component holding it.
 *
 * A reader looking for `success_rate` knows the name before they know which of
 * a dozen components they want it from, so the name is what the list is ordered
 * by and the component is how the repeats of it are told apart.
 */
function byName(left: SignalOption, right: SignalOption): number {
  return left.channel.localeCompare(right.channel) || left.component.localeCompare(right.component)
}

const matching = computed(() => {
  const needle = search.value.trim().toLowerCase()
  const available = props.options.filter((option) => !props.pinned.includes(option.value))
  const found = needle
    ? available.filter((option) => option.value.toLowerCase().includes(needle))
    : available
  // What a variant changed most, first, and alphabetically within that.
  return [...found].sort(
    (left, right) =>
      (props.moved ? magnitude(right.value) - magnitude(left.value) : 0) || byName(left, right),
  )
})

const groups = computed(() =>
  FAMILIES.map(({ key, label }) => ({
    label,
    options: matching.value.filter((option) => option.family === key),
  })).filter((group) => group.options.length > 0),
)

function pin(value: string) {
  if (props.pinned.includes(value)) return
  emit('update:pinned', [...props.pinned, value])
}

function unpin(value: string) {
  emit(
    'update:pinned',
    props.pinned.filter((entry) => entry !== value),
  )
}
</script>

<template>
  <section class="signals" data-test="watch-picker">
    <div class="head">
      <span class="title">Watching</span>
      <button
        v-if="pinned.length"
        class="clear"
        data-test="clear-signals"
        @click="emit('update:pinned', [])"
      >
        clear
      </button>
    </div>

    <ul v-if="chosen.length" class="pinned">
      <li v-for="option in chosen" :key="option.value">
        <button
          class="signal chosen"
          :data-test="`unpin-${option.value}`"
          :title="`Stop watching ${option.value}`"
          @click="unpin(option.value)"
        >
          <el-icon class="mark"><i-view /></el-icon>
          <span class="name">
            <span class="channel">{{ option.channel }}</span>
            <span class="component">{{ option.component }}</span>
          </span>
          <span v-if="movement(option.value)" class="moved" :class="movement(option.value)!.direction">
            {{ movement(option.value)!.label }}
          </span>
          <el-icon class="act"><i-close /></el-icon>
        </button>
      </li>
    </ul>
    <p v-else class="none">Nothing charted yet. Pick a quantity below.</p>

    <el-input
      v-model="search"
      size="small"
      placeholder="Find a quantity"
      clearable
      class="find"
      data-test="signal-search"
    >
      <template #prefix>
        <el-icon><i-search /></el-icon>
      </template>
    </el-input>

    <div class="list">
      <template v-for="group in groups" :key="group.label">
        <p class="group">{{ group.label }}</p>
        <button
          v-for="option in group.options"
          :key="option.value"
          class="signal"
          :data-test="`pin-${option.value}`"
          :title="`Watch ${option.value}`"
          @click="pin(option.value)"
        >
          <el-icon class="mark"><i-plus /></el-icon>
          <span class="name">
            <span class="channel">{{ option.channel }}</span>
            <span class="component">{{ option.component }}</span>
          </span>
          <span v-if="movement(option.value)" class="moved" :class="movement(option.value)!.direction">
            {{ movement(option.value)!.label }}
          </span>
        </button>
      </template>
      <p v-if="!groups.length" class="none">
        {{ options.length ? 'Nothing goes by that name.' : 'Solve the design to see its quantities.' }}
      </p>
    </div>
  </section>
</template>

<style scoped>
.signals {
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1;
  border-top: 1px solid var(--line);
  margin-top: var(--space-2);
  padding-top: var(--space-2);
}
.head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  padding: var(--space-1) var(--space-2);
}
.title {
  font-family: var(--display);
  font-size: var(--text-2xs);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--muted);
  font-weight: 700;
}
.clear {
  border: none;
  background: none;
  padding: 0;
  font-size: var(--text-2xs);
  color: var(--muted);
  text-decoration: underline;
}
.clear:hover { color: var(--green); }

.pinned { list-style: none; margin: 0 0 var(--space-2); padding: 0; display: flex; flex-direction: column; gap: 1px; }
.none { margin: 0 var(--space-2) var(--space-2); font-size: var(--text-2xs); color: var(--muted); }

.signal {
  width: 100%;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 5px var(--space-2);
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  text-align: left;
  color: var(--ink);
  min-width: 0;
}
.signal:hover { background: #e6eae2; }
.signal.chosen { background: var(--green-soft); }
.signal.chosen .mark { color: var(--green); }
.mark { font-size: 12px; color: var(--muted); flex: 0 0 auto; }
.name { display: flex; flex-direction: column; min-width: 0; flex: 1; line-height: 1.25; }
.channel {
  font-family: var(--mono);
  font-size: var(--text-2xs);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.component { font-size: 10px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
/*
 * Which way it went, not whether that is good news. Higher latency and higher
 * throughput are the same arrow, and only the reader knows which they wanted.
 */
.moved { font-family: var(--mono); font-size: 10px; color: var(--muted); flex: 0 0 auto; }
.signal:hover .moved { display: none; }
.act { font-size: 11px; color: var(--muted); flex: 0 0 auto; opacity: 0; }
.signal:hover .act { opacity: 1; }

.find { margin: 0 var(--space-2) var(--space-2); width: auto; }
.list { flex: 1; overflow: auto; min-height: 0; }
.group {
  margin: var(--space-2) var(--space-2) 2px;
  font-family: var(--display);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--muted);
}
</style>
