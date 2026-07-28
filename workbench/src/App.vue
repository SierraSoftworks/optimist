<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useDesign, useDesigns } from './composables/useDesign'
import { useWorkbenchStore } from './stores/workbench'

const route = useRoute()
const router = useRouter()
const store = useWorkbenchStore()

const { data: designs } = useDesigns()

/** The design in the URL, which is the only place it is recorded. */
const design = computed(() => (route.params.design as string | undefined) ?? null)
const { feedStatus } = useDesign(design)

const mode = computed(() => (route.name === 'review' ? 'review' : 'design'))

function go(next: 'design' | 'review') {
  if (!design.value) return
  void router.push({ name: next, params: { design: design.value } })
}

function open(id: string) {
  void router.push({ name: mode.value, params: { design: id } })
}
</script>

<template>
  <div class="shell">
    <header class="bar">
      <button class="brand" data-test="home" @click="router.push({ name: 'welcome' })">
        <el-icon :size="17"><i-data-analysis /></el-icon>
        <span>Optimist</span>
      </button>

      <template v-if="design">
        <el-divider direction="vertical" />
        <el-select
          :model-value="design"
          size="small"
          class="picker"
          data-test="design-picker"
          @change="open"
        >
          <el-option
            v-for="entry in designs ?? []"
            :key="entry.id"
            :label="entry.name"
            :value="entry.id"
            :disabled="!!entry.unreadable"
          />
        </el-select>
      </template>

      <span class="spacer" />

      <template v-if="design">
        <!--
          Two modes rather than a row of panels. Editing a design and judging one
          are different jobs done at different times, and the tool showing only
          what the current job needs is the point of separating them.

          They sit at the right-hand end beside the settings that also belong to
          the whole window. The design's name used to sit here and has gone: it
          is already in the picker two inches to the left, and repeating it cost
          the width that the mode switch now uses.
        -->
        <el-radio-group
          :model-value="mode"
          size="small"
          class="modes"
          data-test="mode-switch"
          @change="(value: string | number | boolean) => go(value as 'design' | 'review')"
        >
          <el-radio-button value="design">
            <el-icon><i-edit-pen /></el-icon>
            <span>Design</span>
          </el-radio-button>
          <el-radio-button value="review">
            <el-icon><i-trend-charts /></el-icon>
            <span>Simulation</span>
          </el-radio-button>
        </el-radio-group>
        <!--
          Behind a gear, because these decide how the answer was produced rather
          than what the answer is. Almost nobody needs to touch them, and having
          them permanently in the bar invited fiddling with the sample count in
          preference to reading the result.
        -->
        <el-popover
          placement="bottom-end"
          trigger="click"
          :width="272"
          popper-class="settings"
        >
          <template #reference>
            <button class="gear" aria-label="Solver settings" data-test="settings">
              <el-icon :size="15"><i-setting /></el-icon>
            </button>
          </template>

          <div class="setting">
            <div class="label">
              <span>Through time</span>
              <p>
                Fill and drain each queue at a finite rate instead of solving for where it
                balances. Shows how long an incident outlasts its cause, and takes
                considerably longer.
              </p>
            </div>
            <el-switch
              :model-value="store.transient"
              size="small"
              data-test="transient-toggle"
              @update:model-value="(value: string | number | boolean) => store.walkThroughTime(value === true)"
            />
          </div>

          <div class="setting">
            <div class="label">
              <span>Samples</span>
              <p>Draws carried through every uncertain quantity.</p>
            </div>
            <el-input-number
              v-model="store.samples"
              :min="64"
              :max="20000"
              :step="500"
              size="small"
              controls-position="right"
              data-test="samples"
            />
          </div>

          <div class="setting">
            <div class="label">
              <span>Horizon</span>
              <p>Steps to advance the model through.</p>
            </div>
            <el-input-number
              v-model="store.horizon"
              :min="1"
              :max="500"
              size="small"
              controls-position="right"
              data-test="horizon"
            />
          </div>

          <div class="setting">
            <div class="label">
              <span>Step</span>
              <p>Seconds each step covers.</p>
            </div>
            <el-input-number
              v-model="store.step"
              :min="0.01"
              :max="3600"
              :step="0.5"
              size="small"
              controls-position="right"
              data-test="step"
            />
          </div>
        </el-popover>

        <el-tooltip :content="`Change feed ${feedStatus}`" placement="bottom">
          <span class="feed" :class="feedStatus" :data-status="feedStatus" data-test="feed" />
        </el-tooltip>
      </template>
    </header>

    <router-view />
  </div>
</template>

<style scoped>
.shell { display: flex; flex-direction: column; height: 100vh; }
.bar {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: 0 var(--space-3);
  height: 46px;
  border-bottom: 1px solid var(--line);
  background: var(--surface-strong);
  flex: 0 0 auto;
  flex-wrap: nowrap;
}
.brand { flex: 0 0 auto; }
.brand {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  border: none;
  background: none;
  font-weight: 700;
  font-size: var(--text-md);
  color: var(--ink);
  padding: 4px 6px;
  border-radius: var(--radius-sm);
}
.brand:hover { background: var(--green-soft); }
.picker { width: 190px; flex: 0 0 auto; }
.modes { flex: 0 0 auto; }
.modes :deep(.el-radio-group) { flex-wrap: nowrap; }
.modes :deep(.el-radio-button__inner) { display: inline-flex; align-items: center; gap: 5px; white-space: nowrap; }
.spacer { flex: 1; }
.gear {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--muted);
}
.gear:hover { background: var(--green-soft); color: var(--green); }
.feed { width: 8px; height: 8px; border-radius: 50%; background: var(--muted); display: inline-block; }
.feed.open { background: #2f9e69; }
.feed.closed { background: var(--danger); }
.feed.connecting { background: var(--caution); }
</style>

<style>
/* The popover renders at the body, so its contents cannot be scoped. */
.settings .setting {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-2) 0;
}
.settings .setting + .setting { border-top: 1px solid var(--line); }
.settings .label { min-width: 0; }
.settings .label span { font-size: var(--text-sm); font-weight: 650; }
.settings .label p { margin: 2px 0 0; font-size: var(--text-2xs); color: var(--muted); line-height: 1.4; }
.settings .el-input-number { width: 96px; flex: 0 0 auto; }
</style>
