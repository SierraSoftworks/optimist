import type {
  ApiErrorBody,
  CreateEdgeInput,
  CreateNodeInput,
  GraphEdge,
  GraphNode,
  Project,
} from './types'

interface ErrorEnvelope {
  error: ApiErrorBody
}

interface CommandResult<T> {
  request_id: string
  project_revision: number
  outcome: { type: string; value: T }
}

export class OptimistApiError extends Error {
  readonly code: string
  readonly advice: string[]

  constructor(
    code: string,
    message: string,
    advice: string[],
  ) {
    super(message)
    this.name = 'OptimistApiError'
    this.code = code
    this.advice = advice
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    headers: {
      Accept: 'application/json',
      ...(init?.body ? { 'Content-Type': 'application/json' } : {}),
      ...init?.headers,
    },
  })
  if (response.ok) return response.json() as Promise<T>

  const body = (await response.json().catch(() => null)) as ErrorEnvelope | null
  if (body?.error) {
    throw new OptimistApiError(body.error.code, body.error.message, body.error.advice)
  }
  throw new OptimistApiError(
    `http_${response.status}`,
    `Optimist returned HTTP ${response.status}.`,
    ['Check that the Optimist server is running and retry.'],
  )
}

export const api = {
  projects: () => request<Project[]>('/api/v1/projects'),
  project: (project: string) => request<Project>(`/api/v1/projects/${project}`),
  nodes: (project: string) => request<GraphNode[]>(`/api/v1/projects/${project}/nodes`),
  edges: (project: string) => request<GraphEdge[]>(`/api/v1/projects/${project}/edges`),
  createProject: (name: string) =>
    request<Project>('/api/v1/projects', {
      method: 'POST',
      body: JSON.stringify({ name }),
    }),
  async createNode(project: Project, input: CreateNodeInput): Promise<GraphNode> {
    const result = await request<CommandResult<GraphNode>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: { type: 'create_node', payload: input },
        }),
      },
    )
    if (result.outcome.type !== 'node_created') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for node creation.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value
  },
  async createEdge(project: Project, input: CreateEdgeInput): Promise<GraphEdge> {
    const result = await request<CommandResult<GraphEdge>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: { type: 'create_edge', payload: input },
        }),
      },
    )
    if (result.outcome.type !== 'edge_created') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for relationship creation.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value
  },
}