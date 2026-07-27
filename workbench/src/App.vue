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
const { data: snapshot, feedStatus } = useDesign(design)

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

        <!--
          Two modes rather than a row of panels. Editing a design and judging one
          are different jobs done at different times, and the tool showing only
          what the current job needs is the point of separating them.
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
            <span>Review</span>
          </el-radio-button>
        </el-radio-group>

        <span class="title">{{ snapshot?.name }}</span>
      </template>

      <span class="spacer" />

      <template v-if="design">
        <el-tooltip content="Draws carried through every uncertain quantity" placement="bottom">
          <div class="control">
            <span>samples</span>
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
        </el-tooltip>
        <el-tooltip content="Steps to advance the model through" placement="bottom">
          <div class="control">
            <span>horizon</span>
            <el-input-number
              v-model="store.horizon"
              :min="1"
              :max="500"
              size="small"
              controls-position="right"
              data-test="horizon"
            />
          </div>
        </el-tooltip>
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
.title {
  font-size: var(--text-sm);
  color: var(--muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;
}
.spacer { flex: 1; }
.control { display: flex; align-items: center; gap: var(--space-1); }
.control span { font-size: var(--text-2xs); color: var(--muted); }
.control :deep(.el-input-number) { width: 92px; }
.feed { width: 8px; height: 8px; border-radius: 50%; background: var(--muted); display: inline-block; }
.feed.open { background: #2f9e69; }
.feed.closed { background: var(--danger); }
.feed.connecting { background: var(--caution); }
</style>
