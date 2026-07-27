<script setup lang="ts">
import { computed } from 'vue'

import type { Comparison, Movement } from '../api/types'

const props = defineProps<{ comparison: Comparison }>()

/**
 * How a constraint moved, in the terms a reader is deciding in.
 *
 * A change that relieves one limit routinely loads another, and reading only the
 * relieved ones is how a proposal gets adopted that moves a problem rather than
 * fixing it. Everything that moved is shown, worst movement first.
 */
type Direction = 'relieved' | 'eased' | 'unchanged' | 'loaded' | 'broken'

function direction(movement: Movement): Direction {
  const before = movement.bound_before
  const after = movement.bound_after
  if (after > 0 && before === 0) return 'broken'
  if (before > 0 && after === 0) return 'relieved'
  const shift = movement.after - movement.before
  if (Math.abs(shift) < 1e-9) return 'unchanged'
  return shift < 0 ? 'eased' : 'loaded'
}

const rows = computed(() =>
  props.comparison.movements
    .map((movement) => ({ movement, direction: direction(movement) }))
    .sort((a, b) => b.movement.after - b.movement.before - (a.movement.after - a.movement.before)),
)

const broke = computed(() => rows.value.filter((row) => row.direction === 'broken'))
</script>

<template>
  <section class="comparison">
    <p v-if="broke.length" class="warning">
      {{ broke.length }} constraint(s) started binding under this change. Relieving one limit
      routinely promotes another, so check whether this is a fix or a move.
    </p>

    <table>
      <thead>
        <tr>
          <th scope="col">Constraint</th>
          <th scope="col" class="numeric">Before</th>
          <th scope="col" class="numeric">After</th>
          <th scope="col">Effect</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in rows" :key="`${row.movement.component}/${row.movement.constraint}`">
          <th scope="row" class="what">
            <span class="component">{{ row.movement.component }}</span>
            <span class="constraint">{{ row.movement.constraint }}</span>
          </th>
          <td class="numeric">{{ row.movement.before.toFixed(3) }}</td>
          <td class="numeric">{{ row.movement.after.toFixed(3) }}</td>
          <td><span class="effect" :class="row.direction">{{ row.direction }}</span></td>
        </tr>
      </tbody>
    </table>

    <p v-if="!rows.length" class="empty">This proposal changes nothing that is measured.</p>
  </section>
</template>

<style scoped>
.comparison { display: flex; flex-direction: column; gap: var(--space-4); }
table { width: 100%; border-collapse: collapse; font-size: var(--text-sm); }
th, td { text-align: left; padding: var(--space-2) var(--space-3); border-bottom: 1px solid var(--line); }
.numeric { text-align: right; font-family: var(--mono); }
.what { display: flex; flex-direction: column; font-weight: 400; }
.component { font-weight: 700; }
.constraint { font-family: var(--mono); font-size: var(--text-xs); color: var(--muted); }
.effect { font-size: var(--text-2xs); font-weight: 650; border-radius: var(--radius-sm); padding: 2px 7px; border: 1px solid var(--line); }
.effect.relieved { background: var(--green-soft); color: var(--green); border-color: var(--green); }
.effect.eased { background: var(--green-soft); color: var(--green); }
.effect.loaded { background: var(--caution-surface); color: var(--caution); border-color: var(--caution-line); }
.effect.broken { background: var(--danger-surface); color: var(--danger); border-color: var(--danger-line); }
.warning {
  margin: 0;
  padding: var(--space-3);
  border: 1px solid var(--danger-line);
  background: var(--danger-surface);
  color: var(--danger);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
}
.empty { color: var(--muted); font-size: var(--text-sm); margin: 0; }
</style>
