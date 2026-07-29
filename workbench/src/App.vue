<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import NewDesignDialog from './components/NewDesignDialog.vue'
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

const creating = ref(false)
const picker = ref<{ blur: () => void } | null>(null)

function startDesign() {
  picker.value?.blur()
  creating.value = true
}

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
          ref="picker"
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

          <!--
            Starting a design belongs beside choosing one: both answer "which
            system am I working on", and putting it here means the welcome
            screen is not the only way in.
          -->
          <template #footer>
            <button class="add" data-test="picker-new-design" @click="startDesign">
              <el-icon :size="13"><i-plus /></el-icon>
              <span>New design</span>
            </button>
          </template>
        </el-select>

        <!--
          Two modes rather than a row of panels. Editing a design and judging one
          are different jobs done at different times, and the tool showing only
          what the current job needs is the point of separating them.

          They read as labels beside the design they belong to rather than as
          controls, because the pair is a statement of which job is in hand and
          not another button competing with the ones inside the view.
        -->
        <el-radio-group
          :model-value="mode"
          size="small"
          class="modes"
          data-test="mode-switch"
          @change="(value: string | number | boolean) => go(value as 'design' | 'review')"
        >
          <el-radio-button value="design">Design</el-radio-button>
          <el-radio-button value="review">Simulation</el-radio-button>
        </el-radio-group>
      </template>

      <span class="spacer" />

      <template v-if="design">
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

    <NewDesignDialog v-model="creating" @created="open" />
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
  font-family: var(--display);
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
.add {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  border: none;
  background: none;
  padding: 2px 0;
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--green);
}
.add:hover { color: var(--ink); }
.modes { flex: 0 0 auto; flex-wrap: nowrap; gap: var(--space-3); }
.modes :deep(.el-radio-button__inner) {
  padding: 4px 0;
  border: none;
  outline: none;
  background: none;
  border-radius: 0;
  box-shadow: none;
  white-space: nowrap;
  text-transform: uppercase;
  letter-spacing: 0.07em;
  font-size: var(--text-xs);
  font-weight: 600;
  color: var(--muted);
}
.modes :deep(.el-radio-button__inner:hover) { color: var(--ink); }
.modes :deep(.el-radio-button.is-active .el-radio-button__original-radio:not(:disabled) + .el-radio-button__inner) {
  background: none;
  border: none;
  box-shadow: none;
  color: var(--green);
  font-weight: 800;
}
.modes :deep(.el-radio-button__original-radio:focus-visible + .el-radio-button__inner) {
  border: none;
  border-radius: var(--radius-sm);
  outline: 2px solid var(--green);
  outline-offset: 2px;
}
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
