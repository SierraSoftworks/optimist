import type {
  AppendObservationInput,
  ApiErrorBody,
  CreateEdgeInput,
  CreateNodeInput,
  EdgeIdentity,
  GraphEdge,
  GraphNode,
  Project,
  ProjectArchive,
  SetStateEstimateInput,
  StateEstimateSlot,
  UpdateNodeInput,
  UpdateEdgeInput,
  Observation,
} from './types'

interface ErrorEnvelope {
  error: ApiErrorBody
}

interface CommandResult<T> {
  request_id: string
  project_revision: number
  outcome: { type: string; value: T }
}

interface PrimitiveEstimate {
  address: {
    project: string
    owner: { kind: 'node'; id: string }
    estimate: string
  }
  slot: { kind: StateEstimateSlot }
  revision: number
  distribution: import('./types').Distribution
  provenance: string[]
}

interface ObservationAppendResult {
  edge: GraphEdge
  observation: Observation
}

function edgeIdentity(edge: GraphEdge): EdgeIdentity {
  return {
    source: edge.source,
    kind: edge.payload.kind,
    destination: edge.destination,
  }
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
  exportProject: (project: string) =>
    request<ProjectArchive>(`/api/v1/projects/${project}/archive`),
  importProject: (archive: ProjectArchive, replace: boolean) =>
    request<Project>(`/api/v1/project-archives?replace=${replace}&yes=${replace}`, {
      method: 'POST',
      body: JSON.stringify(archive),
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
  async updateNode(
    project: Project,
    node: GraphNode,
    input: UpdateNodeInput,
  ): Promise<GraphNode> {
    const result = await request<CommandResult<GraphNode>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            type: 'update_node_metadata',
            payload: {
              id: node.id,
              expected_revision: node.revision,
              ...input,
            },
          },
        }),
      },
    )
    if (result.outcome.type !== 'node_metadata_updated') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for node editing.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value
  },
  async setStateEstimate(
    project: Project,
    node: GraphNode,
    input: SetStateEstimateInput,
  ): Promise<PrimitiveEstimate> {
    const properties =
      node.payload.kind === 'factor' || node.payload.kind === 'outcome'
        ? node.payload.properties
        : null
    if (!properties) {
      throw new OptimistApiError(
        'invalid_estimate_slot',
        'Only factors and outcomes have normalized state estimates.',
        ['Select a factor or outcome and retry.'],
      )
    }
    const current = properties[input.slot]
    const other = properties[input.slot === 'current' ? 'desired' : 'current']
    const estimate = current?.id ?? (other?.id === 'A' ? 'B' : 'A')
    const result = await request<CommandResult<PrimitiveEstimate>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            type: 'set_estimate',
            payload: {
              address: {
                project: project.id,
                owner: { kind: 'node', id: node.id },
                estimate,
              },
              slot: { kind: input.slot },
              distribution: input.distribution,
              provenance: input.provenance,
            },
          },
        }),
      },
    )
    if (result.outcome.type !== 'estimate_set') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for estimate editing.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value
  },
  async updateEdge(
    project: Project,
    edge: GraphEdge,
    input: UpdateEdgeInput,
  ): Promise<GraphEdge> {
    const result = await request<CommandResult<GraphEdge>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            type: 'update_edge_metadata',
            payload: {
              id: edgeIdentity(edge),
              expected_revision: edge.revision,
              ...input,
            },
          },
        }),
      },
    )
    if (result.outcome.type !== 'edge_metadata_updated') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for relationship editing.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value
  },
  async deleteEdge(project: Project, edge: GraphEdge): Promise<GraphEdge> {
    const result = await request<CommandResult<GraphEdge>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            type: 'delete_edge',
            payload: { id: edgeIdentity(edge) },
          },
        }),
      },
    )
    if (result.outcome.type !== 'edge_deleted') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for relationship deletion.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value
  },
  async deleteNode(project: Project, node: GraphNode): Promise<GraphNode> {
    const result = await request<CommandResult<GraphNode>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: { type: 'delete_node', payload: { id: node.id } },
        }),
      },
    )
    if (result.outcome.type !== 'node_deleted') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for node deletion.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value
  },
  async appendObservation(
    project: Project,
    edge: GraphEdge,
    input: AppendObservationInput,
  ): Promise<Observation> {
    const result = await request<CommandResult<ObservationAppendResult>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            type: 'append_observation',
            payload: { edge: edgeIdentity(edge), observation: input },
          },
        }),
      },
    )
    if (result.outcome.type !== 'observation_appended') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for observation creation.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value.observation
  },
}