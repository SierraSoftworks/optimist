<script setup lang="ts">
import { computed } from 'vue'

import type { Bottleneck, Movement } from '../api/types'
import { describeChange, directionOf } from '../domain/change'
import { formatSiNumber } from '../domain/humanNumber'

const props = defineProps<{
  bottlenecks: Bottleneck[]
  /** How each constraint moved under the variant, keyed `component/constraint`. */
  movements?: Record<string, Movement>
  /** Constraints shown before the rest are collapsed into a count. */
  limit?: number
}>()

/**
 * How far below the worst constraint a limit stops being worth the space.
 *
 * A design has as many constraints as it has components, and nearly all of them
 * are idle: a store at a thousandth of its transfer ceiling is not a fact
 * anybody needs in a header. An order of magnitude is the threshold because it
 * is roughly where a constraint stops being reachable by the kind of change
 * somebody makes in one sitting — doubling demand promotes a constraint at half
 * the load, and nothing promotes one at a tenth.
 */
const SHOULDER = 0.1

const shown = computed(() => {
  const ranked = [...props.bottlenecks].sort((a, b) => b.utilisation - a.utilisation)
  const worst = ranked[0]?.utilisation ?? 0
  return ranked.filter(
    // Anything that binds in any draw is kept whatever its mean says. A
    // constraint bound in a tenth of draws has a mean that looks comfortable and
    // an outage that does not.
    (entry) => entry.probability_of_binding > 0 || entry.utilisation >= worst * SHOULDER,
  )
})

const visible = computed(() => shown.value.slice(0, props.limit ?? 4))
const hidden = computed(() => shown.value.length - visible.value.length)

function movementOf(entry: Bottleneck): Movement | undefined {
  return props.movements?.[`${entry.component}/${entry.constraint}`]
}

/** How a constraint moved, once it has moved enough to be worth saying. */
function shift(entry: Bottleneck) {
  const movement = movementOf(entry)
  if (!movement) return null
  const label = describeChange(movement.before, movement.after)
  const direction = directionOf(movement.before, movement.after)
  const relieved = movement.bound_before > 0 && movement.bound_after === 0
  const introduced = movement.bound_before === 0 && movement.bound_after > 0
  if (!label || !direction) return relieved || introduced ? { relieved, introduced } : null
  return {
    // Named for what it does to the constraint rather than for its sign. More
    // load on a limit is worse whichever quantity produced it, which is the one
    // place in this workbench where a colour can honestly take a side.
    direction: direction === 'up' ? 'worse' : 'better',
    label,
    relieved,
    introduced,
  }
}

/** Utilisation as a bar, with anything past the limit pinned at full. */
function fill(utilisation: number): string {
  return `${Math.min(Math.max(utilisation, 0), 1) * 100}%`
}

/**
 * How loaded a constraint is, in the form that stays readable.
 *
 * A percentage is right up to a few times over, and falls apart past that:
 * "9999%" is four digits a reader has to count before learning what "×100"
 * says at a glance.
 */
function load(utilisation: number): string {
  return utilisation >= 10
    ? `\u00d7${formatSiNumber(utilisation, 2)}`
    : `${(utilisation * 100).toFixed(0)}%`
}
</script>

<template>
  <div v-if="visible.length" class="limits" data-test="limit-cards">
    <article
      v-for="entry in visible"
      :key="`${entry.component}/${entry.constraint}`"
      class="limit"
      :class="{ binding: entry.probability_of_binding > 0 }"
      :data-test="`limit-${entry.component}-${entry.constraint}`"
      :title="entry.summary"
    >
      <div class="what">
        <span class="component">{{ entry.component }}</span>
        <span class="constraint">{{ entry.constraint }}</span>
      </div>
      <div class="reading">
        <span class="load">{{ load(entry.utilisation) }}</span>
        <span v-if="shift(entry)?.label" class="shift" :class="shift(entry)!.direction">
          {{ shift(entry)!.label }}
        </span>
      </div>
      <div class="gauge" aria-hidden="true">
        <span class="fill" :style="{ width: fill(entry.utilisation) }" />
      </div>
      <span v-if="shift(entry)?.relieved" class="note good">no longer binds</span>
      <span v-else-if="shift(entry)?.introduced" class="note bad">starts binding</span>
    </article>

    <span v-if="hidden > 0" class="rest" :title="`${hidden} more, all far from their limit`">
      +{{ hidden }}
    </span>
  </div>
</template>

<style scoped>
.limits {
  display: flex;
  align-items: stretch;
  gap: var(--space-2);
  flex-wrap: wrap;
  justify-content: flex-end;
}
.limit {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 104px;
  padding: 5px var(--space-2) 6px;
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface);
}
.limit.binding { border-color: var(--danger-line); background: var(--danger-surface); }
.what { display: flex; flex-direction: column; line-height: 1.15; min-width: 0; }
.component { font-size: var(--text-2xs); font-weight: 700; }
.constraint { font-family: var(--mono); font-size: 10px; color: var(--muted); }
.reading { display: flex; align-items: baseline; gap: var(--space-1); }
.load { font-family: var(--mono); font-size: var(--text-md); line-height: 1.1; }
.limit.binding .load { color: var(--danger); }
.shift { font-family: var(--mono); font-size: 10px; }
/*
 * Named for what it does to the constraint rather than for its sign. More load
 * on a limit is worse whichever quantity produced it, which is the one place in
 * this workbench where a colour can honestly take a side.
 */
.shift.better { color: var(--green); }
.shift.worse { color: var(--danger); }
.gauge { height: 3px; border-radius: 2px; background: var(--line); overflow: hidden; }
.fill { display: block; height: 100%; background: var(--green); }
.limit.binding .fill { background: var(--danger); }
.note { font-size: 9px; text-transform: uppercase; letter-spacing: 0.04em; }
.note.good { color: var(--green); }
.note.bad { color: var(--danger); }
.rest { align-self: center; font-size: var(--text-2xs); color: var(--muted); }
</style>
