<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import { X } from '@lucide/vue'

const props = defineProps<{ open: boolean; pending: boolean }>()
const emit = defineEmits<{ close: []; submit: [name: string] }>()
const name = ref('')
const input = ref<HTMLInputElement>()

watch(
  () => props.open,
  async (open) => {
    if (!open) return
    name.value = ''
    await nextTick()
    input.value?.focus()
  },
)

function submit() {
  const value = name.value.trim()
  if (value) emit('submit', value)
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="dialog-backdrop" @click.self="emit('close')">
      <form class="dialog" aria-labelledby="create-project-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">New workspace</span>
            <h2 id="create-project-title">Create project</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <label>
          Project name
          <input ref="input" v-model="name" autocomplete="off" placeholder="Delivery reliability" required />
        </label>
        <footer>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending || !name.trim()">
            {{ pending ? 'Creating…' : 'Create project' }}
          </button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
