<script setup lang="ts">
import { computed } from 'vue'

import type { Bottleneck, Movement } from '../api/types'
import { formatSiNumber } from '../domain/humanNumber'

interface ServiceLevelReading {
  /** The share of operations which currently satisfy the SLI. */
  current: number
  /** The same share in the design without the selected intervention. */
  baseline?: number
  /** The configured SLO, or absent when perfection is the only reference. */
  objective?: number
}

const props = defineProps<{
  bottlenecks: Bottleneck[]
  /** How each constraint moved under the variant, keyed `component/constraint`. */
  movements?: Record<string, Movement>
  /** Actual SLI readings, keyed `component/constraint`. */
  serviceLevels?: Record<string, ServiceLevelReading>
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

function serviceLevelOf(entry: Bottleneck): ServiceLevelReading | undefined {
  return props.serviceLevels?.[`${entry.component}/${entry.constraint}`]
}

/** How a constraint moved, once it has moved enough to be worth saying. */
function shift(entry: Bottleneck) {
  const movement = movementOf(entry)
  const serviceLevel = serviceLevelOf(entry)
  const relieved = !!movement && movement.bound_before > 0 && movement.bound_after === 0
  const introduced = !!movement && movement.bound_before === 0 && movement.bound_after > 0
  const before = serviceLevel ? serviceLevel.baseline : movement?.before
  const after = serviceLevel?.current ?? movement?.after
  if (before === undefined || after === undefined || before === after) {
    return relieved || introduced ? { relieved, introduced } : null
  }
  const difference = after - before
  return {
    direction: serviceLevel
      ? difference > 0 ? 'better' : 'worse'
      : difference > 0 ? 'worse' : 'better',
    label: serviceLevel ? signedPercent(difference) : signedRatio(difference),
    relieved,
    introduced,
  }
}

function signedPercent(value: number): string {
  return `${value > 0 ? '+' : '\u2212'}${percent(Math.abs(value))}`
}

function signedRatio(value: number): string {
  return `${value > 0 ? '+' : '\u2212'}${ratio(Math.abs(value))}`
}

function percent(value: number): string {
  const scaled = value * 100
  const digits = scaled > 0 && scaled < 0.1 ? 2 : 1
  return `${Number(scaled.toFixed(digits))}%`
}

function ratio(value: number): string {
  const text = value < 1000 ? Number(value.toPrecision(3)).toString() : formatSiNumber(value, 3)
  return `${text}x`
}

function reading(entry: Bottleneck): string {
  return serviceLevelOf(entry)
    ? percent(serviceLevelOf(entry)!.current)
    : ratio(entry.utilisation)
}

/** The displayed quantity as a bar, pinned to its natural upper bound. */
function fill(entry: Bottleneck): string {
  const value = serviceLevelOf(entry)?.current ?? entry.utilisation
  return `${Math.min(Math.max(value, 0), 1) * 100}%`
}

function underperforming(entry: Bottleneck): boolean {
  const serviceLevel = serviceLevelOf(entry)
  if (!serviceLevel) return entry.probability_of_binding > 0
  return serviceLevel.current < (serviceLevel.objective ?? 1)
}
</script>

<template>
  <div v-if="visible.length" class="limits" data-test="limit-cards">
    <article
      v-for="entry in visible"
      :key="`${entry.component}/${entry.constraint}`"
      class="limit"
      :class="{ binding: underperforming(entry), healthy: serviceLevelOf(entry) && !underperforming(entry) }"
      :data-test="`limit-${entry.component}-${entry.constraint}`"
      :title="entry.summary"
    >
      <div class="what">
        <span class="component">{{ entry.component }}</span>
        <span class="constraint">{{ entry.constraint }}</span>
      </div>
      <div class="reading">
        <span class="load">{{ reading(entry) }}</span>
        <span v-if="shift(entry)?.label" class="shift" :class="shift(entry)!.direction">
          {{ shift(entry)!.label }}
        </span>
      </div>
      <div class="gauge" aria-hidden="true">
        <span class="fill" :style="{ width: fill(entry) }" />
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
.limit.healthy .load { color: var(--green); }
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
