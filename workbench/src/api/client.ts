import type {
  AppendObservationInput,
  CorrectObservationInput,
  ApiErrorBody,
  CreateEdgeInput,
  CreateNodeInput,
  EdgeIdentity,
  EdgeEstimateSlot,
  Estimate,
  EstimateSource,
  EstimateSourceInput,
  Evidence,
  EvidenceInput,
  GraphEdge,
  ImpedimentAnalysis,
  GraphNode,
  Project,
  ProjectArchive,
  ProjectDependenceModel,
  SetStateEstimateInput,
  Scenario,
  ScenarioAnalysis,
  ScenarioDraft,
  ServerHealth,
  SquiggleAssessmentResult,
  SquiggleEstimateDefinition,
  EstimateSupport,
  StructuralAnalysis,
  SetInterventionEstimateInput,
  InterventionEstimateSlot,
  SetEdgeEstimateInput,
  SetMeasurementCalibrationInput,
  SetEffectProfileInput,
  UpdateCausalEffectInput,
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
    owner: { kind: 'node'; id: string } | { kind: 'edge'; id: EdgeIdentity }
    estimate: string
  }
  slot: { kind: StateEstimateSlot } | EdgeEstimateSlot | InterventionEstimateSlot
  revision: number
  distribution?: import('./types').Distribution
  quantity?: import('./types').QuantityDefinition
  source: EstimateSource
  provenance: string[]
  uncertainty?: import('./types').EstimateUncertainty
}

function estimateCommand(
  address: PrimitiveEstimate['address'],
  slot: PrimitiveEstimate['slot'],
  source: EstimateSourceInput,
  provenance: string[],
  uncertainty?: import('./types').EstimateUncertainty,
) {
  return {
    type: 'set_squiggle_estimate',
    payload: { address, slot, definition: source.definition, provenance, uncertainty },
  }
}

const estimateIdAlphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_.'

function encodeEstimateId(value: number) {
  if (value === 0) return estimateIdAlphabet[0]!
  let encoded = ''
  while (value > 0) {
    encoded = estimateIdAlphabet[value % 64]! + encoded
    value = Math.floor(value / 64)
  }
  return encoded
}

function interventionEstimate(node: GraphNode, input: SetInterventionEstimateInput) {
  if (node.payload.kind !== 'intervention') return null
  const slot = input.slot
  if (slot.kind === 'duration') return node.payload.properties.duration
  if (slot.kind === 'probability_of_success') {
    return node.payload.properties.probability_of_success
  }
  return node.payload.properties.costs.find(
    (cost) => cost.dimension === slot.value.trim(),
  )?.value ?? null
}

function nextInterventionEstimateId(node: GraphNode) {
  if (node.payload.kind !== 'intervention') return 'A'
  const used = new Set([
    ...node.payload.properties.costs.map((cost) => cost.value.id),
    ...(node.payload.properties.duration ? [node.payload.properties.duration.id] : []),
    ...(node.payload.properties.probability_of_success
      ? [node.payload.properties.probability_of_success.id]
      : []),
  ])
  for (let value = 0; value < Number.MAX_SAFE_INTEGER; value += 1) {
    const id = encodeEstimateId(value)
    if (!used.has(id)) return id
  }
  throw new OptimistApiError(
    'estimate_identifier_space_exhausted',
    'The intervention has exhausted its estimate identifier space.',
    ['Remove an unused estimate and retry.'],
  )
}

function edgeEstimate(edge: GraphEdge, slot: EdgeEstimateSlot) {
  if (edge.payload.kind === 'contributes' || edge.payload.kind === 'changes') {
    if (slot.kind === 'response') return edge.payload.properties.response
    if (slot.kind === 'lag') return edge.payload.properties.lag
  }
  if (edge.payload.kind === 'blocks' && slot.kind === 'degree') {
    return edge.payload.properties.degree
  }
  return null
}

function nextEdgeEstimateId(edge: GraphEdge) {
  const used = new Set<string>()
  if (edge.payload.kind === 'contributes' || edge.payload.kind === 'changes') {
    used.add(edge.payload.properties.response.id)
    if (edge.payload.properties.lag) used.add(edge.payload.properties.lag.id)
  } else if (edge.payload.kind === 'blocks') {
    used.add(edge.payload.properties.degree.id)
  }
  for (let value = 0; value < Number.MAX_SAFE_INTEGER; value += 1) {
    const id = encodeEstimateId(value)
    if (!used.has(id)) return id
  }
  throw new OptimistApiError(
    'estimate_identifier_space_exhausted',
    'The relationship has exhausted its estimate identifier space.',
    ['Remove its optional lag and retry.'],
  )
}

interface ObservationAppendResult {
  edge: GraphEdge
  observation: Observation
}

interface EvidenceMutationResult {
  node: GraphNode
  evidence: Evidence
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

  const rawBody = await response.text().catch(() => '')
  const body = parseErrorEnvelope(rawBody)
  if (body?.error) {
    throw new OptimistApiError(body.error.code, body.error.message, body.error.advice)
  }
  const rejection = deserializeRejection(rawBody)
  if (rejection) {
    throw new OptimistApiError(
      'invalid_request_payload',
      `The server rejected the “${rejection.field}” field in the submitted ${rejection.subject}. The workbench and server data formats may be out of sync.`,
      [
        'Refresh the page before retrying so the workbench matches the running server.',
        `If this continues, report the rejected field path: ${rejection.path}.`,
      ],
    )
  }
  throw new OptimistApiError(
    `http_${response.status}`,
    rawBody.trim()
      ? `Optimist returned HTTP ${response.status}: ${rawBody.trim()}`
      : `Optimist returned HTTP ${response.status}.`,
    ['Check that the Optimist server is running, refresh the page, and retry.'],
  )
}

function parseErrorEnvelope(value: string): ErrorEnvelope | null {
  try {
    return JSON.parse(value) as ErrorEnvelope
  } catch {
    return null
  }
}

function deserializeRejection(value: string) {
  const match = value.match(/Failed to deserialize the JSON body[^:]*:\s+(.+?): unknown field `([^`]+)`/)
  if (!match) return null
  const path = match[1]!
  return {
    path,
    field: match[2]!,
    subject: path.includes('properties.response')
      ? 'relationship estimate'
      : 'change',
  }
}

export const api = {
  health: () => request<ServerHealth>('/api/v1/health'),
  projects: () => request<Project[]>('/api/v1/projects'),
  project: (project: string) => request<Project>(`/api/v1/projects/${project}`),
  nodes: (project: string) => request<GraphNode[]>(`/api/v1/projects/${project}/nodes`),
  edges: (project: string) => request<GraphEdge[]>(`/api/v1/projects/${project}/edges`),
  structuralAnalysis: (project: string) =>
    request<StructuralAnalysis>(`/api/v1/projects/${project}/analysis/structure`),
  impedimentAnalysis: (project: string) =>
    request<ImpedimentAnalysis>(`/api/v1/projects/${project}/analysis/impediments`),
  assessSquiggle: (project: string, definition: SquiggleEstimateDefinition, support: EstimateSupport) =>
    request<SquiggleAssessmentResult>(`/api/v1/projects/${project}/analysis/squiggle-assessment`, {
      method: 'POST',
      body: JSON.stringify({ definition, support }),
    }),
  scenarios: (project: string) =>
    request<Scenario[]>(`/api/v1/projects/${project}/scenarios`),
  scenarioAnalysis: (project: string, scenario: string) =>
    request<ScenarioAnalysis>(`/api/v1/projects/${project}/scenarios/${scenario}/analysis`),
  /** Reads the residual dependence document, which most projects do not have. */
  async dependence(project: string): Promise<ProjectDependenceModel | null> {
    try {
      return await request<ProjectDependenceModel>(`/api/v1/projects/${project}/dependence`)
    } catch (error) {
      if (error instanceof OptimistApiError && error.code === 'dependence_not_found') return null
      throw error
    }
  },
  async setDependence(
    project: Project,
    model: ProjectDependenceModel,
  ): Promise<ProjectDependenceModel> {
    const result = await request<CommandResult<ProjectDependenceModel>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: { type: 'set_project_dependence', payload: { model } },
        }),
      },
    )
    if (result.outcome.type !== 'project_dependence_set') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for dependence editing.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value
  },
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
  async createScenario(project: Project, scenario: ScenarioDraft): Promise<Scenario> {
    const result = await request<CommandResult<Scenario>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: { type: 'create_scenario', payload: { scenario } },
        }),
      },
    )
    if (result.outcome.type !== 'scenario_created') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for scenario creation.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value
  },
  async updateScenario(
    project: Project,
    current: Scenario,
    scenario: ScenarioDraft,
  ): Promise<Scenario> {
    const result = await request<CommandResult<Scenario>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            type: 'update_scenario',
            payload: {
              id: current.id,
              expected_revision: current.revision,
              scenario,
            },
          },
        }),
      },
    )
    if (result.outcome.type !== 'scenario_updated') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for scenario editing.',
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
  async setNodeQuantityState(
    project: Project,
    node: GraphNode,
    input: import('./types').SetNodeQuantityStateInput,
  ): Promise<GraphNode> {
    const result = await request<CommandResult<GraphNode>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            type: 'set_node_quantity_state',
            payload: { node: node.id, expected_revision: node.revision, ...input },
          },
        }),
      },
    )
    if (result.outcome.type !== 'node_quantity_state_set') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for native state configuration.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value
  },
  async setStateRelation(
    project: Project,
    node: GraphNode,
    input: import('./types').SetStateRelationInput,
  ): Promise<GraphNode> {
    const result = await request<CommandResult<GraphNode>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            type: 'set_state_relation',
            payload: { node: node.id, expected_revision: node.revision, ...input },
          },
        }),
      },
    )
    if (result.outcome.type !== 'state_relation_set') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for node equation editing.',
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
    if (node.payload.kind === 'metric' && input.slot !== 'current') {
      throw new OptimistApiError(
        'invalid_estimate_slot',
        'Metrics only have a current native quantity estimate.',
        ['Choose the current slot and retry.'],
      )
    }
    if (node.payload.kind !== 'factor' && node.payload.kind !== 'outcome' && node.payload.kind !== 'metric') {
      throw new OptimistApiError(
        'invalid_estimate_slot',
        'This node does not have a current quantity estimate.',
        ['Select a factor, outcome, or metric and retry.'],
      )
    }
    if (node.payload.kind !== 'metric' && !node.native_state) {
      throw new OptimistApiError(
        'missing_quantity_state',
        'Configure this node quantity before authoring state estimates.',
        ['Set its unit, support, and operational definition, then retry.'],
      )
    }
    const current = node.payload.kind === 'metric'
      ? node.payload.properties.current
      : input.slot === 'current' ? node.native_state!.current : node.native_state!.forecast
    const other = node.payload.kind === 'metric'
      ? null
      : input.slot === 'current' ? node.native_state!.forecast : node.native_state!.current
    const estimate = current?.id ?? (other?.id === 'A' ? 'B' : 'A')
    const result = await request<CommandResult<PrimitiveEstimate>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            ...estimateCommand(
              { project: project.id, owner: { kind: 'node', id: node.id }, estimate },
              { kind: input.slot },
              input.source,
              input.provenance,
              input.uncertainty,
            ),
          },
        }),
      },
    )
    if (result.outcome.type !== 'squiggle_estimate_set') {
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
  async setMeasurementCalibration(
    project: Project,
    edge: GraphEdge,
    input: SetMeasurementCalibrationInput,
  ): Promise<GraphEdge> {
    const result = await request<CommandResult<GraphEdge>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            type: 'set_measurement_calibration',
            payload: {
              edge: edgeIdentity(edge),
              expected_revision: edge.revision,
              calibration: input.calibration,
            },
          },
        }),
      },
    )
    if (result.outcome.type !== 'measurement_calibration_set') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for measurement calibration.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value
  },
  async setEffectProfile(
    project: Project,
    edge: GraphEdge,
    input: SetEffectProfileInput,
  ): Promise<GraphEdge> {
    const result = await request<CommandResult<GraphEdge>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            type: 'set_effect_profile',
            payload: {
              edge: edgeIdentity(edge),
              expected_revision: edge.revision,
              profile: input.profile,
            },
          },
        }),
      },
    )
    if (result.outcome.type !== 'effect_profile_set') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for the intervention effect profile.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value
  },
  async updateCausalEffect(
    project: Project,
    edge: GraphEdge,
    input: UpdateCausalEffectInput,
  ): Promise<GraphEdge> {
    const result = await request<CommandResult<GraphEdge>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            type: 'update_causal_effect',
            payload: {
              edge: edgeIdentity(edge),
              expected_revision: edge.revision,
              ...input,
            },
          },
        }),
      },
    )
    if (result.outcome.type !== 'causal_effect_updated') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for the causal relationship.',
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
  async correctObservation(
    project: Project,
    edge: GraphEdge,
    input: CorrectObservationInput,
  ): Promise<Observation> {
    const result = await request<CommandResult<ObservationAppendResult>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            type: 'correct_observation',
            payload: { edge: edgeIdentity(edge), ...input },
          },
        }),
      },
    )
    if (result.outcome.type !== 'observation_corrected') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for observation correction.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value.observation
  },
  async setInterventionEstimate(
    project: Project,
    node: GraphNode,
    input: SetInterventionEstimateInput,
  ): Promise<PrimitiveEstimate> {
    const existing = interventionEstimate(node, input)
    const estimate = existing?.id ?? nextInterventionEstimateId(node)
    const result = await request<CommandResult<PrimitiveEstimate>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: estimateCommand(
            { project: project.id, owner: { kind: 'node', id: node.id }, estimate },
            input.slot,
            input.source,
            input.provenance,
            input.uncertainty,
          ),
        }),
      },
    )
    if (result.outcome.type !== 'squiggle_estimate_set') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for intervention estimate editing.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value
  },
  async removeInterventionEstimate(
    project: Project,
    node: GraphNode,
    estimate: Estimate,
  ): Promise<PrimitiveEstimate> {
    const result = await request<CommandResult<PrimitiveEstimate>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            type: 'remove_estimate',
            payload: {
              address: {
                project: project.id,
                owner: { kind: 'node', id: node.id },
                estimate: estimate.id,
              },
            },
          },
        }),
      },
    )
    if (result.outcome.type !== 'estimate_removed') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for intervention estimate removal.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value
  },
  async createEvidence(
    project: Project,
    node: GraphNode,
    input: EvidenceInput,
  ): Promise<Evidence> {
    const result = await request<CommandResult<EvidenceMutationResult>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            type: 'create_evidence',
            payload: { node: node.id, ...input },
          },
        }),
      },
    )
    if (result.outcome.type !== 'evidence_created') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for evidence creation.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value.evidence
  },
  async updateEvidence(
    project: Project,
    node: GraphNode,
    evidence: Evidence,
    input: EvidenceInput,
  ): Promise<Evidence> {
    const result = await request<CommandResult<EvidenceMutationResult>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            type: 'update_evidence',
            payload: {
              node: node.id,
              evidence_id: evidence.id,
              expected_revision: evidence.revision,
              ...input,
            },
          },
        }),
      },
    )
    if (result.outcome.type !== 'evidence_updated') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for evidence editing.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value.evidence
  },
  async deleteEvidence(
    project: Project,
    node: GraphNode,
    evidence: Evidence,
  ): Promise<Evidence> {
    const result = await request<CommandResult<EvidenceMutationResult>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            type: 'delete_evidence',
            payload: {
              node: node.id,
              evidence_id: evidence.id,
              expected_revision: evidence.revision,
            },
          },
        }),
      },
    )
    if (result.outcome.type !== 'evidence_deleted') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for evidence deletion.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value.evidence
  },
  async setEdgeEstimate(
    project: Project,
    edge: GraphEdge,
    input: SetEdgeEstimateInput,
  ): Promise<PrimitiveEstimate> {
    const existing = edgeEstimate(edge, input.slot)
    const estimate = existing?.id ?? nextEdgeEstimateId(edge)
    const result = await request<CommandResult<PrimitiveEstimate>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: estimateCommand(
            {
              project: project.id,
              owner: { kind: 'edge', id: edgeIdentity(edge) },
              estimate,
            },
            input.slot,
            input.source,
            input.provenance,
            input.uncertainty,
          ),
        }),
      },
    )
    if (result.outcome.type !== 'squiggle_estimate_set') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for relationship estimate editing.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value
  },
  async removeEdgeEstimate(
    project: Project,
    edge: GraphEdge,
    estimate: Estimate,
  ): Promise<PrimitiveEstimate> {
    const result = await request<CommandResult<PrimitiveEstimate>>(
      `/api/v1/projects/${project.id}/commands`,
      {
        method: 'POST',
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: project.revision,
          command: {
            type: 'remove_estimate',
            payload: {
              address: {
                project: project.id,
                owner: { kind: 'edge', id: edgeIdentity(edge) },
                estimate: estimate.id,
              },
            },
          },
        }),
      },
    )
    if (result.outcome.type !== 'estimate_removed') {
      throw new OptimistApiError(
        'unexpected_command_result',
        'Optimist returned an unexpected result for relationship estimate removal.',
        ['Confirm the workbench and server versions match.'],
      )
    }
    return result.outcome.value
  },
}