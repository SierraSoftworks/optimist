import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router'

import WelcomeView from './views/WelcomeView.vue'

/**
 * Addressable state lives in the URL.
 *
 * A design, a mode, and what is selected within it are all things somebody sends
 * to a colleague, so they belong in a link rather than in memory. Controls that
 * only change how hard the solver works are left out: they describe how the
 * screen was produced rather than what is on it.
 *
 * The two working views are loaded on demand. Between them they pull in a graph
 * library and a code editor, neither of which is needed to show somebody the
 * list of designs they can open.
 */
const routes: RouteRecordRaw[] = [
  { path: '/', name: 'welcome', component: WelcomeView },
  {
    path: '/d/:design/design/:selected?',
    name: 'design',
    component: () => import('./views/DesignView.vue'),
    props: true,
  },
  {
    path: '/d/:design/review/:intervention?',
    name: 'review',
    component: () => import('./views/ReviewView.vue'),
    props: true,
  },
  // An unknown path is more likely a stale link than an attack, so it lands on
  // the picker rather than an error.
  { path: '/:pathMatch(.*)*', redirect: '/' },
]

export const router = createRouter({ history: createWebHistory(), routes })
