export type NodeKind = 'outcome' | 'metric' | 'factor' | 'intervention'
export type EdgeKind =
  | 'contributes'
  | 'measures'
  | 'changes'
  | 'requires'
  | 'part_of'
  | 'blocks'
  | 'conflicts_with'
  | 'synergizes_with'

export interface Project {
  id: string
  name: string
  revision: number
}

export interface Distribution {
  type: 'point' | 'normal' | 'log_normal' | 'beta' | 'scaled_beta'
  value?: number
  mean?: number
  standard_deviation?: number
  location?: number
  scale?: number
  alpha?: number
  beta?: number
  lower?: number
  upper?: number
}

export interface Estimate {
  id: string
  revision: number
  distribution: Distribution
  provenance?: string[]
}

export interface Evidence {
  id: number
  revision: number
  summary: string
  source: string | null
}

export interface Observation {
  id: number
  revision: number
  value: number
  unit: string
  observed_at: string
  source: string
  measurement_standard_deviation: number | null
  supersedes: number | null
}

export interface AppendObservationInput {
  value: number
  unit: string
  observed_at: string
  source: string
  measurement_standard_deviation: number | null
}

export type NodePayload =
  | {
      kind: 'outcome'
      properties: {
        direction: 'maximize' | 'minimize' | 'target_range'
        current: Estimate | null
        desired: Estimate | null
        evidence: Evidence[]
      }
    }
  | {
      kind: 'metric'
      properties: { unit: string; aggregation: string | null }
    }
  | {
      kind: 'factor'
      properties: {
        current: Estimate | null
        desired: Estimate | null
        controllable: boolean
        evidence: Evidence[]
      }
    }
  | {
      kind: 'intervention'
      properties: {
        costs: Array<{ dimension: string; value: Estimate }>
        duration: Estimate | null
        probability_of_success: Estimate | null
        acceptance_criteria: string[]
      }
    }

export interface GraphNode {
  id: string
  revision: number
  name: string
  normalized_name: string
  title: string
  description: string
  aliases: string[]
  metadata: Record<string, unknown>
  payload: NodePayload
}

export interface GraphEdge {
  source: string
  source_kind: NodeKind
  destination: string
  destination_kind: NodeKind
  revision: number
  description: string
  metadata: Record<string, unknown>
  payload:
    | {
        kind: 'measures'
        properties: {
          polarity: 'higher_is_better' | 'lower_is_better' | 'target_range'
          observations: Observation[]
        }
      }
    | { kind: Exclude<EdgeKind, 'measures'>; properties?: Record<string, unknown> }
}

export interface EdgeIdentity {
  source: string
  kind: EdgeKind
  destination: string
}

export interface CreateNodeInput {
  name: string
  title: string
  payload: NodePayload
}

export interface UpdateNodeInput {
  title: string
  description: string
  metadata: Record<string, unknown>
}

export type StateEstimateSlot = 'current' | 'desired'

export interface SetStateEstimateInput {
  slot: StateEstimateSlot
  distribution: Distribution
  provenance: string[]
}

export interface UpdateEdgeInput {
  description: string
  metadata: Record<string, unknown>
}

export type EditableEdgePayload =
  | {
      kind: 'contributes' | 'changes'
      properties: {
        effect: Estimate
        lag: Estimate | null
        mechanism: string
        evidence: string[]
      }
    }
  | {
      kind: 'measures'
      properties: {
        polarity: 'higher_is_better' | 'lower_is_better' | 'target_range'
        observations: []
      }
    }
  | { kind: 'part_of' }
  | {
      kind: 'requires'
      properties: { hard: boolean; satisfaction_threshold: number | null }
    }
  | {
      kind: 'blocks'
      properties: { degree: Estimate }
    }
  | { kind: 'conflicts_with' | 'synergizes_with' }

export interface CreateEdgeInput {
  source: string
  destination: string
  payload: EditableEdgePayload
}

export interface ApiErrorBody {
  code: string
  message: string
  advice: string[]
}

export interface ProjectArchive {
  schema_version: number
  project: Project
  files: Record<string, string>
  summary: {
    entities: number
    edges: number
    scenarios: number
  }
}