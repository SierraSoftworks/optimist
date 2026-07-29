import { createApp } from 'vue'
import { VueQueryPlugin } from '@tanstack/vue-query'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import {
  Document,
  MagicStick,
  Box,
  Check,
  Close,
  Connection,
  DataAnalysis,
  Delete,
  Download,
  EditPen,
  Filter,
  InfoFilled,
  Loading,
  Plus,
  Right,
  Search,
  Select,
  Setting,
  TrendCharts,
  Upload,
  View,
  Warning,
  WarningFilled,
} from '@element-plus/icons-vue'
import '@fontsource-variable/montserrat/index.css'
import '@fontsource-variable/saira/index.css'
import '@fontsource-variable/fira-code/index.css'
import 'element-plus/dist/index.css'
import './style.css'
import App from './App.vue'
import { router } from './router'

const app = createApp(App)

// Registered by hand rather than by looping over the icon package, which pulls
// several hundred components into the bundle to use a dozen. A missing one shows
// up as a resolution warning and a blank space, so the list is worth keeping in
// step with the templates.
const icons = {
  Box,
  Check,
  Close,
  Connection,
  DataAnalysis,
  Delete,
  Document,
  Download,
  EditPen,
  Filter,
  InfoFilled,
  Loading,
  MagicStick,
  Plus,
  Right,
  Search,
  Select,
  Setting,
  TrendCharts,
  Upload,
  View,
  Warning,
  WarningFilled,
}
for (const [name, component] of Object.entries(icons)) {
  app.component(`i-${name.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase()}`, component)
}

app.use(createPinia()).use(router).use(ElementPlus).use(VueQueryPlugin).mount('#app')
