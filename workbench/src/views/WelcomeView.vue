<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'

import NewDesignDialog from '../components/NewDesignDialog.vue'
import { useDesigns } from '../composables/useDesign'

const router = useRouter()
const { data: designs } = useDesigns()

const creating = ref(false)

function open(design: string) {
  void router.push({ name: 'design', params: { design } })
}
</script>

<template>
  <div class="welcome">
    <div class="inner">
      <header>
        <div>
          <h1>Designs</h1>
          <p>Every system this server is holding.</p>
        </div>
        <el-button type="primary" data-test="new-design" @click="creating = true">
          <el-icon><i-plus /></el-icon>
          <span>New design</span>
        </el-button>
      </header>

      <el-empty
        v-if="designs && !designs.length"
        description="Nothing here yet."
        :image-size="80"
      >
        <el-button type="primary" @click="creating = true">Create the first one</el-button>
      </el-empty>

      <ul v-else class="designs">
        <li v-for="entry in designs ?? []" :key="entry.id">
          <button
            class="card"
            :disabled="!!entry.unreadable"
            :data-test="`open-${entry.id}`"
            @click="open(entry.id)"
          >
            <span class="name">{{ entry.name }}</span>
            <span class="summary">{{ entry.summary }}</span>
            <el-alert
              v-if="entry.unreadable"
              type="error"
              :closable="false"
              show-icon
              :title="entry.unreadable"
            />
          </button>
        </li>
      </ul>
    </div>

    <NewDesignDialog v-model="creating" @created="open" />
  </div>
</template>

<style scoped>
.welcome { flex: 1; overflow: auto; padding: var(--space-6) var(--space-5); }
.inner { max-width: 860px; margin: 0 auto; }
header { display: flex; align-items: flex-start; justify-content: space-between; margin-bottom: var(--space-5); }
h1 { font-size: var(--text-2xl); margin: 0; }
header p { color: var(--muted); font-size: var(--text-sm); margin: var(--space-1) 0 0; }
.designs { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: var(--space-3); }
.card {
  width: 100%;
  text-align: left;
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface-strong);
  padding: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  transition: border-color 0.12s ease, box-shadow 0.12s ease;
}
.card:hover:not(:disabled) { border-color: var(--green); box-shadow: 0 1px 6px rgb(36 87 70 / 12%); }
.card:disabled { cursor: not-allowed; opacity: 0.8; }
.name { font-weight: 700; }
.summary { color: var(--muted); font-size: var(--text-xs); display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }
</style>
