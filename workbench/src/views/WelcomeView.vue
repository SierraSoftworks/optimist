<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'

import { api } from '../api/client'
import { useDesigns } from '../composables/useDesign'

const router = useRouter()
const { data: designs, refetch } = useDesigns()

const creating = ref(false)
const id = ref('')
const name = ref('')
const summary = ref('')
const failure = ref<string | null>(null)

/** A directory name, so the same rule the server enforces is applied here first. */
function slug(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '')
}

async function create() {
  failure.value = null
  const identifier = slug(id.value || name.value)
  if (!identifier) return
  try {
    await api.create(identifier, name.value || identifier, summary.value)
    await refetch()
    creating.value = false
    void router.push({ name: 'design', params: { design: identifier } })
  } catch (error) {
    failure.value = (error as Error).message
  }
}

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

    <el-dialog v-model="creating" title="New design" width="460px">
      <el-form label-position="top" size="small" @submit.prevent="create">
        <el-form-item label="Name">
          <el-input v-model="name" placeholder="Checkout" data-test="design-name" autofocus />
        </el-form-item>
        <el-form-item label="Identifier">
          <el-input
            :model-value="slug(id || name)"
            placeholder="checkout"
            data-test="design-id"
            @update:model-value="(value: string) => (id = value)"
          />
          <p class="hint">This becomes the directory the design is stored in.</p>
        </el-form-item>
        <el-form-item label="Summary">
          <el-input v-model="summary" type="textarea" :rows="2" placeholder="What it is for." />
        </el-form-item>
        <el-alert v-if="failure" type="error" :closable="false" show-icon :title="failure" />
      </el-form>
      <template #footer>
        <el-button size="small" @click="creating = false">Cancel</el-button>
        <el-button
          type="primary"
          size="small"
          :disabled="!slug(id || name)"
          data-test="create-design"
          @click="create"
        >
          Create
        </el-button>
      </template>
    </el-dialog>
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
.hint { color: var(--muted); font-size: var(--text-2xs); margin: 2px 0 0; }
</style>
