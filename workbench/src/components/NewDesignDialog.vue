<script setup lang="ts">
import { useQueryClient } from '@tanstack/vue-query'
import { ref, watch } from 'vue'

import { api } from '../api/client'

const open = defineModel<boolean>({ required: true })
const emit = defineEmits<{ created: [design: string] }>()

const client = useQueryClient()

const id = ref('')
const name = ref('')
const summary = ref('')
const failure = ref<string | null>(null)
const saving = ref(false)

/** A directory name, so the same rule the server enforces is applied here first. */
function slug(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '')
}

watch(open, (showing) => {
  if (showing) return
  id.value = ''
  name.value = ''
  summary.value = ''
  failure.value = null
})

async function create() {
  const identifier = slug(id.value || name.value)
  if (!identifier || saving.value) return
  failure.value = null
  saving.value = true
  try {
    await api.create(identifier, name.value || identifier, summary.value)
    await client.invalidateQueries({ queryKey: ['designs'] })
    open.value = false
    emit('created', identifier)
  } catch (error) {
    failure.value = (error as Error).message
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <el-dialog v-model="open" title="New design" width="460px">
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
      <el-button size="small" @click="open = false">Cancel</el-button>
      <el-button
        type="primary"
        size="small"
        :loading="saving"
        :disabled="!slug(id || name)"
        data-test="create-design"
        @click="create"
      >
        Create
      </el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.hint { color: var(--muted); font-size: var(--text-2xs); margin: 2px 0 0; }
</style>
