import { onScopeDispose, watch, type Ref } from 'vue'
import { storeToRefs } from 'pinia'
import type { Project } from '../api/types'
import { useWorkbenchStore } from '../stores/workbench'
import { parseRoute, routePath } from '../domain/route'

/**
 * Binds the open project and the current view to the address bar.
 *
 * The address bar is read once on load and again on every history entry, and it
 * is rewritten whenever the workbench moves. Writing compares against the
 * current path first, which is what keeps the two directions from chasing each
 * other: applying a popped entry leaves the path already correct, so the watcher
 * that would push it back has nothing to do.
 *
 * A move is pushed only when the address bar was naming a project the workbench
 * can actually show. Bootstrapping from `/`, or correcting a link that names a
 * project this server does not have, replaces instead, so a dead link cannot
 * leave an entry that redirects again every time it is revisited.
 *
 * @param projects Projects this server offers, used to tell a navigation from a
 *   correction. An empty list while the query is in flight reads as a
 *   correction, which is the safe direction: no history is created for a state
 *   the workbench has not confirmed it can show.
 */
export function useWorkbenchRoute(projects: Ref<Project[]>) {
  const store = useWorkbenchStore()
  const { mode, selectedProjectId } = storeToRefs(store)

  function adopt() {
    const route = parseRoute(window.location.pathname)
    if (route.projectId && route.projectId !== selectedProjectId.value) {
      store.selectProject(route.projectId)
    }
    mode.value = route.mode
  }

  adopt()
  window.addEventListener('popstate', adopt)
  onScopeDispose(() => window.removeEventListener('popstate', adopt))

  watch([selectedProjectId, mode], () => {
    const path = routePath({ projectId: selectedProjectId.value, mode: mode.value })
    if (path === window.location.pathname) return
    const leaving = parseRoute(window.location.pathname).projectId
    const navigating = leaving !== null && projects.value.some((project) => project.id === leaving)
    window.history[navigating ? 'pushState' : 'replaceState'](null, '', path)
  })
}
