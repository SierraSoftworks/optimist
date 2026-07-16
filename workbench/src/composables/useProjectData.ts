import { computed, type Ref } from 'vue'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from '../api/client'
import type { CreateEdgeInput, CreateNodeInput, Project, ProjectArchive } from '../api/types'

export function useProjects() {
  return useQuery({ queryKey: ['projects'], queryFn: api.projects })
}

export function useProject(projectId: Ref<string | null>) {
  const enabled = computed(() => Boolean(projectId.value))
  return useQuery({
    queryKey: computed(() => ['project', projectId.value]),
    queryFn: () => api.project(projectId.value!),
    enabled,
  })
}

export function useGraph(projectId: Ref<string | null>) {
  const enabled = computed(() => Boolean(projectId.value))
  const nodes = useQuery({
    queryKey: computed(() => ['nodes', projectId.value]),
    queryFn: () => api.nodes(projectId.value!),
    enabled,
  })
  const edges = useQuery({
    queryKey: computed(() => ['edges', projectId.value]),
    queryFn: () => api.edges(projectId.value!),
    enabled,
  })
  return { nodes, edges }
}

export function useCreateProject() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: api.createProject,
    onSuccess: (project) => {
      queryClient.setQueryData<Project[]>(['projects'], (projects = []) => [
        ...projects,
        project,
      ])
    },
  })
}

export function useImportProject() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ archive, replace }: { archive: ProjectArchive; replace: boolean }) =>
      api.importProject(archive, replace),
    onSuccess: async () => {
      await queryClient.invalidateQueries()
    },
  })
}

export function useCreateNode(project: Ref<Project | undefined>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: CreateNodeInput) => api.createNode(project.value!, input),
    onSuccess: async () => {
      const id = project.value!.id
      await Promise.all([
        queryClient.refetchQueries({ queryKey: ['project', id] }),
        queryClient.refetchQueries({ queryKey: ['nodes', id] }),
      ])
    },
  })
}

export function useCreateEdge(project: Ref<Project | undefined>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: CreateEdgeInput) => api.createEdge(project.value!, input),
    onSuccess: async () => {
      const id = project.value!.id
      await Promise.all([
        queryClient.refetchQueries({ queryKey: ['project', id] }),
        queryClient.refetchQueries({ queryKey: ['edges', id] }),
      ])
    },
  })
}