import { createApp } from 'vue'
import { VueQueryPlugin } from '@tanstack/vue-query'
import { createPinia } from 'pinia'
import '@fontsource-variable/manrope/index.css'
import '@fontsource/ibm-plex-mono/400.css'
import '@fontsource/ibm-plex-mono/500.css'
import '@quri/squiggle-components/full.css'
import './style.css'
import App from './App.vue'

createApp(App).use(createPinia()).use(VueQueryPlugin).mount('#app')
