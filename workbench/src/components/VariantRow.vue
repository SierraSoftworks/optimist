<script setup lang="ts">
import { computed } from 'vue'

import type { Intervention, RunningSolve } from '../api/types'

const props = defineProps<{
  /** The variant this row stands for, or null for the design as it stands. */
  entry: Intervention | null
  /** Whether this is the variant on show. */
  active: boolean
  /** The solve running for it, if the server is working on one. */
  solving: RunningSolve | null
}>()

const emit = defineEmits<{
  choose: []
  edit: []
  remove: []
}>()

const name = computed(() => props.entry?.name ?? 'As designed')

const summary = computed(
  () => props.entry?.summary || 'The design as it stands, with nothing proposed.',
)

/**
 * How far the ring is filled, or null while nothing is known yet.
 *
 * A solve that has announced itself but not taken a pass has no honest figure to
 * show, and drawing an empty ring for it would read as stuck rather than
 * starting. The row spins instead until there is something to say.
 */
const filled = computed(() => {
  const solve = props.solving
  if (!solve || solve.fraction <= 0) return null
  return Math.min(100, Math.round(solve.fraction * 100))
})

const detail = computed(() => {
  const solve = props.solving
  if (!solve) return null
  const where =
    solve.steps > 1 ? `Step ${solve.step} of ${solve.steps}, pass ${solve.pass}` : `Pass ${solve.pass}`
  return {
    what: solve.kind === 'comparison' ? 'Weighing against the design as it stands' : 'Solving',
    where,
    waiting: solve.moving ? `${solve.moving.component}.${solve.moving.channel}` : null,
  }
})
</script>

<template>
  <el-popover trigger="hover" placement="right" :width="300" :show-after="350">
    <template #reference>
      <button
        class="variant"
        :class="{ active }"
        :data-test="entry ? `variant-${entry.id}` : 'variant-baseline'"
        @click="emit('choose')"
      >
        <!--
          The indicator takes the icon's place rather than sitting beside it, so
          a rail of variants does not change width the moment one starts solving.
        -->
        <span class="mark" :data-test="solving ? 'variant-solving' : undefined">
          <span
            v-if="solving && filled !== null"
            class="ring"
            :style="{ '--filled': `${filled * 3.6}deg` }"
            role="progressbar"
            :aria-valuenow="filled"
            :aria-label="`Solving ${name}`"
          />
          <el-icon v-else-if="solving" class="spin"><i-loading /></el-icon>
          <el-icon v-else-if="entry"><i-magic-stick /></el-icon>
          <el-icon v-else><i-document /></el-icon>
        </span>
        <span class="label">{{ name }}</span>
        <span v-if="entry" class="actions">
          <el-icon class="action" :aria-label="`Edit ${name}`" @click.stop="emit('edit')">
            <i-edit-pen />
          </el-icon>
          <el-popconfirm :title="`Remove ${name}?`" @confirm="emit('remove')">
            <template #reference>
              <el-icon class="action" :aria-label="`Remove ${name}`" @click.stop>
                <i-delete />
              </el-icon>
            </template>
          </el-popconfirm>
        </span>
      </button>
    </template>

    <div class="about" :data-test="`about-${entry?.id ?? 'baseline'}`">
      <p class="name">{{ name }}</p>
      <p class="summary">{{ summary }}</p>
      <ul v-if="entry?.overrides.length" class="rebinds">
        <li v-for="override in entry.overrides" :key="override.name">
          <code>{{ override.name }}</code> becomes <code>{{ override.expression }}</code>
        </li>
      </ul>
      <div v-if="detail" class="progress">
        <p class="what">{{ detail.what }}{{ filled === null ? '' : ` — ${filled}%` }}</p>
        <p class="where">{{ detail.where }}</p>
        <p v-if="detail.waiting" class="where">Waiting on {{ detail.waiting }}</p>
      </div>
    </div>
  </el-popover>
</template>

<style scoped>
.variant {
  width: 100%;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 6px var(--space-2);
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  text-align: left;
  font-size: var(--text-sm);
  color: var(--ink);
  min-width: 0;
}
.variant:hover { background: #e6eae2; }
.variant.active { background: var(--green-soft); color: var(--green); font-weight: 650; }
/* Fixed so the row is the same width whether it is solving or not. */
.mark {
  flex: 0 0 auto;
  width: 13px;
  height: 13px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  color: var(--muted);
}
.variant.active .mark { color: var(--green); }
.ring {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: conic-gradient(currentColor var(--filled), var(--line) 0);
  color: var(--green);
}
.spin { animation: spin 1s linear infinite; color: var(--green); }
@keyframes spin { to { transform: rotate(360deg); } }
.label { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.actions { display: none; gap: var(--space-1); }
.variant:hover .actions { display: flex; }
.action { font-size: 12px; color: var(--muted); }
.action:hover { color: var(--green); }

.about { display: flex; flex-direction: column; gap: var(--space-1); }
.about .name { margin: 0; font-weight: 650; font-size: var(--text-sm); }
.about .summary { margin: 0; font-size: var(--text-xs); color: var(--muted); }
.rebinds { margin: 0; padding-left: var(--space-3); font-size: var(--text-xs); color: var(--muted); }
.progress {
  margin-top: var(--space-1);
  padding-top: var(--space-1);
  border-top: 1px solid var(--line);
}
.progress p { margin: 0; font-size: var(--text-xs); }
.progress .what { color: var(--green); font-weight: 650; }
.progress .where { color: var(--muted); font-variant-numeric: tabular-nums; }
</style>
