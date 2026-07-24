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

export interface ServerHealth {
  status: 'ok' | 'degraded'
  version: string
  persistence: {
    state: 'idle' | 'pending' | 'error'
    error?: string
  }
}

export interface Distribution {
  type: 'point' | 'normal' | 'log_normal' | 'beta' | 'scaled_beta' | 'empirical'
  value?: number
  mean?: number
  standard_deviation?: number
  location?: number
  scale?: number
  alpha?: number
  beta?: number
  lower?: number
  upper?: number
  samples?: number[]
}

export interface Estimate {
  id: string
  revision: number
  distribution?: Distribution
  quantity?: QuantityDefinition
  source: EstimateSource
  provenance?: string[]
  uncertainty?: EstimateUncertainty
}

export interface EstimateUncertainty {
  epistemic?: string
  process?: string
  measurement?: string
}

export interface QuantityDefinition {
  unit: string
  dimension?: Unit
  aggregation: string | null
  support?: QuantitySupport
  operational_definition?: string
  reference_time?: string | null
  resolution_source?: string | null
}

export interface QuantityState {
  quantity: QuantityDefinition
  current?: Estimate | null
  forecast?: Estimate | null
}

export interface SetNodeQuantityStateInput {
  quantity: QuantityDefinition
}

export interface EstimateSource {
  type: 'squiggle'
  definition: SquiggleEstimateDefinition
}

export interface EstimateSourceInput {
  type: 'squiggle'
  definition: SquiggleEstimateDefinition
}

export interface Evidence {
  id: number
  revision: number
  summary: string
  source: string | null
}

export interface EvidenceInput {
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

export type MeasurementCalibration =
  | { type: 'linear'; state_zero: number; state_one: number }
  | {
      type: 'target_range'
      outer_lower: number
      ideal_lower: number
      ideal_upper: number
      outer_upper: number
    }

export type QuantitySupport =
  | { type: 'real' }
  | { type: 'non_negative' }
  | { type: 'bounded'; lower: number; upper: number }

export interface AppendObservationInput {
  value: number
  unit: string
  observed_at: string
  source: string
  measurement_standard_deviation: number | null
}

export interface CorrectObservationInput {
  observation_id: number
  value: number
}

export type NodePayload =
  | {
      kind: 'outcome'
      properties: {
        direction: 'maximize' | 'minimize' | 'target_range'
        evidence: Evidence[]
      }
    }
  | {
      kind: 'metric'
      properties: {
        quantity: QuantityDefinition
        current?: Estimate | null
      }
    }
  | {
      kind: 'factor'
      properties: {
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
  native_state?: QuantityState | null
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
        kind: 'contributes' | 'changes'
        properties: {
          response: LinearResponse
          transience?: EffectTransience | null
          lag: Estimate | null
          mechanism: string
          evidence: string[]
        }
      }
    | {
        kind: 'blocks'
        properties: { degree: Estimate }
      }
    | {
        kind: 'measures'
        properties: {
          polarity: 'higher_is_better' | 'lower_is_better' | 'target_range'
          calibration?: MeasurementCalibration
          observations: Observation[]
        }
      }
    | {
        kind: Exclude<EdgeKind, 'contributes' | 'changes' | 'blocks' | 'measures'>
        properties?: Record<string, unknown>
      }
}

export interface LinearResponse {
  source_change: number
  source_unit: Unit
  destination_change: Estimate
  destination_unit: Unit
}

/** How a transient effect subsides once its hold window ends. */
export type EffectRelease =
  | { type: 'immediate' }
  | { type: 'linear'; over: Estimate }
  | { type: 'exponential'; half_life: Estimate }

export interface EffectAftereffect {
  hold?: Estimate | null
  release: EffectRelease
}

export interface EffectProfile {
  ramp?: Estimate | null
  hold?: Estimate | null
  release: EffectRelease
  aftereffect?: EffectAftereffect | null
}

/** Temporal shape and rebound of one intervention effect. */
export interface EffectTransience {
  profile: EffectProfile
  rebound?: Estimate | null
}

export interface EdgeIdentity {
  source: string
  kind: EdgeKind
  destination: string
}

export interface AnalysisRevisionKey {
  project: string
  graph_revision: number
  scenario: [string, number] | null
  dependence_revision: number | null
}

export interface StronglyConnectedComponent {
  nodes: string[]
  edges: EdgeIdentity[]
  is_feedback: boolean
}

export interface ElementaryCycle {
  nodes: string[]
  edges: EdgeIdentity[]
}

export interface StructuralAnalysis {
  revision: AnalysisRevisionKey
  components: StronglyConnectedComponent[]
  cycles: ElementaryCycle[]
  cycles_truncated: boolean
  limits: {
    maximum_cycle_length: number
    maximum_cycles: number
  }
}

export interface MonteCarloConfig {
  seed: number
  minimum_samples: number
  maximum_samples: number
  absolute_tolerance: number
  relative_tolerance: number
}

export interface ScenarioObjective {
  outcome_id: string
  direction: 'maximize' | 'minimize'
  importance: number
}

export interface ScenarioDraft {
  name: string
  title: string
  rationale: string
  objectives: ScenarioObjective[]
  planning_horizon: number
  budgets: Array<{ unit: Record<string, number>; amount: number }>
  candidate_interventions: string[]
  monte_carlo: MonteCarloConfig
  scalar_preferences?: Array<{
    unit: Record<string, number>
    utility_per_unit: number
  }>
}

export interface Scenario extends ScenarioDraft {
  id: string
  revision: number
}

export interface MonteCarloEstimate {
  mean: number | null
  variance: number | null
  mean_standard_error: number | null
  variance_standard_error: number | null
}

export interface MonteCarloDiagnostics {
  seed: number
  attempted_samples: number
  valid_samples: number
  invalid_samples: {
    non_finite_primitive: number
    non_finite_result: number
  }
  criterion: MonteCarloConfig
  status: 'converged' | 'maximum_samples_reached' | 'insufficient_valid_samples'
}

export type Unit = Record<string, number>

export type EstimateSupport =
  | 'real'
  | 'non_negative'
  | 'probability'
  | 'signed'
  | { bounded: { lower: number; upper: number } }

export interface SquiggleEstimateDefinition {
  source: string
  seed: number
  sample_count: number
  target_unit: Unit
}

export interface SquiggleEstimateAssessment {
  family: string
  mean: number | null
  variance: number | null
  p05: number
  p50: number
  p95: number
  seed: number
  sample_count: number
}

export interface SquiggleAssessmentResult {
  assessment: SquiggleEstimateAssessment
  effective_distribution: Distribution
  predictive_checks: {
    attempted_draws: number
    valid_draws: number
    invalid_draws: number
    support_violation_draws: number
    support_violation_probability: number
    support_compatible: boolean
    support_requirement: string
    representative_outcomes: Array<{ percentile: number; value: number }>
  }
}

export interface ObjectiveProjection {
  outcome: string
  direction: 'maximize' | 'minimize'
  importance: number
  reachable: boolean
  baseline: MonteCarloEstimate
  final_state: MonteCarloEstimate
  improvement: MonteCarloEstimate
  trajectory: ObjectiveTrajectoryPoint[]
}

export interface ObjectiveTrajectoryPoint {
  period: number
  state: MonteCarloEstimate
  improvement: MonteCarloEstimate
}

export interface InterventionProjection {
  intervention: string
  prerequisites: string[]
  blocking_requirements: InterventionRequirement[]
  synergies: string[]
  conflicts: string[]
  execution_duration: MonteCarloEstimate
  execution_success: MonteCarloEstimate
  objectives: ObjectiveProjection[]
  improvement_covariance: Array<Array<number | null>>
  clamped_state_updates: number
  diagnostics: MonteCarloDiagnostics
}

export interface ScenarioAnalysis {
  revision: AnalysisRevisionKey
  planning_horizon: number
  candidates: InterventionProjection[]
}

export interface InterventionRequirement {
  dependent: string
  prerequisite: string
  hard: boolean
  satisfaction_threshold: number | null
}

export interface InterventionExecutionStep {
  intervention: string
  duration: Distribution | null
  probability_of_success: Distribution | null
}

export interface ImpedimentCandidate {
  intervention: string
  execution_steps: InterventionExecutionStep[]
  blocking_requirements: InterventionRequirement[]
  synergies: string[]
  conflicts: string[]
  expected_duration: number
  expected_success_probability: number
}

export interface ImpedimentAnalysis {
  revision: AnalysisRevisionKey
  candidates: ImpedimentCandidate[]
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

export type StateEstimateSlot = 'current' | 'forecast'

export interface SetStateEstimateInput {
  slot: StateEstimateSlot
  source: EstimateSourceInput
  provenance: string[]
  uncertainty?: EstimateUncertainty
}

export type InterventionEstimateSlot =
  | { kind: 'cost'; value: string }
  | { kind: 'duration' }
  | { kind: 'probability_of_success' }

export interface SetInterventionEstimateInput {
  slot: InterventionEstimateSlot
  source: EstimateSourceInput
  provenance: string[]
  uncertainty?: EstimateUncertainty
}

export type EdgeEstimateSlot = { kind: 'response' | 'lag' | 'degree' }

export interface SetEdgeEstimateInput {
  slot: EdgeEstimateSlot
  source: EstimateSourceInput
  provenance: string[]
  uncertainty?: EstimateUncertainty
}

export interface UpdateEdgeInput {
  description: string
  metadata: Record<string, unknown>
}

export interface SetMeasurementCalibrationInput {
  calibration: MeasurementCalibration | null
}

/** Authored release form; every duration is a Squiggle program in `duration` units. */
export type EffectReleaseInput =
  | { type: 'immediate' }
  | { type: 'linear'; over: SquiggleEstimateDefinition }
  | { type: 'exponential'; half_life: SquiggleEstimateDefinition }

export interface EffectAftereffectInput {
  magnitude: SquiggleEstimateDefinition
  hold: SquiggleEstimateDefinition | null
  release: EffectReleaseInput
}

export interface EffectProfileInput {
  ramp: SquiggleEstimateDefinition | null
  hold: SquiggleEstimateDefinition | null
  release: EffectReleaseInput
  aftereffect: EffectAftereffectInput | null
}

export interface SetEffectProfileInput {
  profile: EffectProfileInput | null
}

export type EditableEdgePayload =
  | {
      kind: 'contributes' | 'changes'
      properties: {
        response: LinearResponse
        lag: Estimate | null
        mechanism: string
        evidence: string[]
      }
    }  | {
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
  description?: string
  dependence?: unknown
  entities: Array<{
    schema_version: number
    base_project_revision: number
    node: GraphNode
    outgoing_edges?: GraphEdge[]
  }>
  scenarios?: Array<{
    schema_version: number
    base_project_revision: number
    scenario: Scenario
  }>
}