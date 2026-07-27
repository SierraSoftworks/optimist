import { createApp } from 'vue'
import { VueQueryPlugin } from '@tanstack/vue-query'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import {
  Close,
  Connection,
  DataAnalysis,
  Delete,
  EditPen,
  Plus,
  Right,
  TrendCharts,
} from '@element-plus/icons-vue'
import '@fontsource-variable/manrope/index.css'
import '@fontsource/ibm-plex-mono/400.css'
import '@fontsource/ibm-plex-mono/500.css'
import 'element-plus/dist/index.css'
import './style.css'
import App from './App.vue'
import { router } from './router'

const app = createApp(App)

// Registered by hand rather than by looping over the icon package, which pulls
// several hundred components into the bundle to use eight of them.
const icons = { Close, Connection, DataAnalysis, Delete, EditPen, Plus, Right, TrendCharts }
for (const [name, component] of Object.entries(icons)) {
  app.component(`i-${name.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase()}`, component)
}

app.use(createPinia()).use(router).use(ElementPlus).use(VueQueryPlugin).mount('#app')
