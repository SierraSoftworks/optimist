<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Activity, AlertTriangle, CheckCircle2, Gauge, Goal, Pencil, Plus, Sigma, Trash2, Wrench } from '@lucide/vue'
import type {
  Distribution,
  Estimate,
  GraphEdge,
  GraphNode,
  InterventionEstimateSlot,
  Observation,
  QuantityDefinition,
} from '../api/types'
import { calibratedState, calibrationLabel } from '../domain/measurementCalibration'
import { readinessLabel, simulationReadiness } from '../domain/simulationReadiness'
import { edgeMetadataLabel } from '../domain/edgePresentation'
import { canOwnRelation, nodeQuantity, nodeRelation } from '../domain/stateRelation'

const props = defineProps<{ node: GraphNode | null; edges: GraphEdge[] }>()
const emit = defineEmits<{
  edit: []
  estimate: []
  quantity: []
  relation: []
  relationship: [edge: GraphEdge]
  observe: [edge: GraphEdge]
  correct: [edge: GraphEdge, observation: Observation]
  interventionEstimate: [slot: InterventionEstimateSlot]
  delete: []
}>()
const confirmDelete = ref(false)
const equation = computed(() => (props.node ? nodeRelation(props.node) : null))
const equationEligible = computed(() => Boolean(props.node && canOwnRelation(props.node)))
const readiness = computed(() => props.node ? simulationReadiness(props.node) : null)
const incidentEdges = computed(() =>
  props.node
    ? props.edges.filter(
        (edge) => edge.source === props.node?.id || edge.destination === props.node?.id,
      )
    : [],
)
const canEditNativeState = computed(() => {
  const kind = props.node?.payload.kind
  return kind === 'factor' || kind === 'outcome' || kind === 'metric'
})
const measurementEdges = computed(() =>
  props.node?.payload.kind === 'metric'
    ? props.edges.filter(
        (edge) => edge.source === props.node?.id && edge.payload.kind === 'measures',
      )
    : [],
)
watch(() => props.node?.id, () => { confirmDelete.value = false })

const kindLabel = computed(() => props.node?.payload.kind ?? '')
const Icon = computed(() => {
  switch (props.node?.payload.kind) {
    case 'outcome':
      return Goal
    case 'metric':
      return Gauge
    case 'intervention':
      return Wrench
    default:
      return Activity
  }
})

function stateEstimate(node: GraphNode, slot: 'current' | 'forecast') {
  if (node.payload.kind !== 'outcome' && node.payload.kind !== 'factor') return null
  return slot === 'current'
    ? node.native_state?.current ?? null
    : node.native_state?.forecast ?? null
}

function distributionLabel(node: GraphNode, slot: 'current' | 'forecast') {
  const value = stateEstimate(node, slot)
  return value ? formatEstimate(value) : 'Not set'
}

function formatEstimate(value: Estimate) {
  if (value.distribution) return formatDistribution(value.distribution)
  return `Squiggle · ${value.source.definition.source.trim().split('\n')[0]}`
}

function formatDistribution(value: Distribution) {
  if (value.type === 'point') return `Point · ${value.value}`
  if (value.type === 'beta') return `Beta · α ${value.alpha}, β ${value.beta}`
  if (value.type === 'scaled_beta') {
    return `Scaled Beta · [${value.lower}, ${value.upper}]`
  }
  if (value.type === 'normal') return `Normal · μ ${value.mean}, σ ${value.standard_deviation}`
  if (value.type === 'empirical') return `Empirical · ${(value.samples ?? []).length.toLocaleString()} samples`
  return `LogNormal · μ ${value.location}, σ ${value.scale}`
}

function supportLabel(quantity: QuantityDefinition) {
  if (quantity.support?.type === 'bounded') {
    return `${quantity.support.lower} to ${quantity.support.upper}`
  }
  return quantity.support?.type.replaceAll('_', ' ') ?? 'Any real value'
}

function replacement(edge: GraphEdge, observation: Observation) {
  if (edge.payload.kind !== 'measures') return null
  return edge.payload.properties.observations.find(
    (candidate) => candidate.supersedes === observation.id,
  ) ?? null
}
</script>

<template>
  <aside class="inspector" aria-label="Selection inspector">
    <template v-if="node">
      <header class="inspector-header">
        <span class="kind-icon" :data-kind="node.payload.kind"><component :is="Icon" :size="18" /></span>
        <div>
          <span class="eyebrow">{{ kindLabel }} · {{ node.id }}</span>
          <h2>{{ node.title }}</h2>
        </div>
      </header>

      <div class="inspector-actions">
        <button type="button" class="secondary-button" @click="emit('edit')"><Pencil :size="14" /> Details</button>
        <button
          v-if="node.payload.kind === 'metric' || node.native_state"
          type="button"
          class="secondary-button"
          @click="emit('estimate')"
        ><Sigma :size="14" /> Estimate</button>
        <button
          v-if="canEditNativeState"
          type="button"
          class="secondary-button"
          @click="emit('quantity')"
        ><Gauge :size="14" /> {{ nodeQuantity(node) ? 'State type' : 'Native state' }}</button>
      </div>

      <section class="readiness-panel" :data-level="readiness?.level">
        <CheckCircle2 v-if="readiness?.level === 'ready'" :size="17" />
        <AlertTriangle v-else :size="17" />
        <div>
          <strong>{{ readiness ? readinessLabel(readiness) : '' }}</strong>
          <div v-if="readiness?.issues.length" class="readiness-actions">
            <button
              v-for="issue in readiness.issues"
              :key="issue.key"
              type="button"
              @click="issue.key === 'quantity_state' ? emit('quantity') : issue.key === 'current_state' ? emit('estimate') : emit('interventionEstimate', { kind: issue.key === 'duration' ? 'duration' : 'probability_of_success' })"
            >{{ issue.label }} <Pencil :size="11" /></button>
          </div>
        </div>
      </section>

      <p v-if="node.description" class="description">{{ node.description }}</p>
      <p v-else class="muted">No description has been added.</p>

      <section v-if="node.payload.kind === 'outcome' || node.payload.kind === 'factor'" class="inspector-section model-section">
        <h3>State model <span v-if="node.native_state">{{ node.native_state.quantity.unit }}</span></h3>
        <div v-if="node.native_state" class="model-facts">
          <div><span>Support</span><strong>{{ supportLabel(node.native_state.quantity) }}</strong></div>
          <div><span>Aggregation</span><strong>{{ node.native_state.quantity.aggregation ?? 'Not set' }}</strong></div>
          <div><span>{{ node.payload.kind === 'factor' ? 'Control' : 'Direction' }}</span><strong>{{ node.payload.kind === 'factor' ? (node.payload.properties.controllable ? 'Direct' : 'Indirect') : node.payload.properties.direction.replaceAll('_', ' ') }}</strong></div>
        </div>
        <div v-else class="model-empty">
          <Gauge :size="17" />
          <div><strong>No state quantity</strong><span>Define the unit and support before adding estimates.</span></div>
          <button type="button" class="secondary-button" @click="emit('quantity')">Configure</button>
        </div>
        <dl>
          <div><dt>Current</dt><dd>{{ distributionLabel(node, 'current') }}</dd></div>
          <div><dt>Forecast</dt><dd>{{ distributionLabel(node, 'forecast') }}</dd></div>
        </dl>
        <p v-if="node.native_state?.quantity.operational_definition" class="model-definition">{{ node.native_state.quantity.operational_definition }}</p>
      </section>

      <section v-if="equationEligible" class="inspector-section equation-section">
        <h3>Node equation</h3>
        <template v-if="equation">
          <pre class="equation-source">{{ equation.source }}</pre>
          <p class="equation-note">
            Computed from its parents each period, replacing the responses on the relationships
            reaching it.
          </p>
        </template>
        <p v-else class="muted">
          Composed from the proportional responses on its incoming relationships.
        </p>
        <button type="button" class="secondary-button" @click="emit('relation')">
          <Sigma :size="13" /> {{ equation ? 'Edit equation' : 'Add equation' }}
        </button>
      </section>

      <section v-if="(node.payload.kind === 'factor' || node.payload.kind === 'outcome') && node.payload.properties.evidence.length" class="inspector-section">
        <h3>Evidence <span>{{ node.payload.properties.evidence.length }}</span></h3>
        <ul class="evidence-list">
          <li v-for="item in node.payload.properties.evidence" :key="item.id">
            <strong>{{ item.summary }}</strong>
            <span>{{ item.source ?? 'No source recorded' }}</span>
          </li>
        </ul>
      </section>

      <section v-if="node.payload.kind === 'metric'" class="inspector-section model-section">
        <h3>Metric model <span>{{ node.payload.properties.quantity.unit }}</span></h3>
        <div class="model-facts">
          <div><span>Support</span><strong>{{ supportLabel(node.payload.properties.quantity) }}</strong></div>
          <div><span>Aggregation</span><strong>{{ node.payload.properties.quantity.aggregation ?? 'Not set' }}</strong></div>
          <div><span>Series</span><strong>{{ measurementEdges.length }}</strong></div>
        </div>
        <dl>
          <div><dt>Current estimate</dt><dd>{{ node.payload.properties.current ? formatEstimate(node.payload.properties.current) : 'Not set' }}</dd></div>
          <div v-if="node.payload.properties.quantity.reference_time"><dt>Reference time</dt><dd>{{ node.payload.properties.quantity.reference_time }}</dd></div>
          <div v-if="node.payload.properties.quantity.resolution_source"><dt>Resolution source</dt><dd>{{ node.payload.properties.quantity.resolution_source }}</dd></div>
        </dl>
        <p v-if="node.payload.properties.quantity.operational_definition" class="model-definition">{{ node.payload.properties.quantity.operational_definition }}</p>
      </section>

      <section v-if="node.payload.kind === 'metric'" class="inspector-section">
        <h3>Observation series <span>{{ measurementEdges.length }}</span></h3>
        <div v-for="edge in measurementEdges" :key="edge.destination" class="observation-series">
          <div class="observation-series-header">
            <div>
              <strong>{{ edge.destination }}</strong>
              <span>{{ edge.payload.kind === 'measures' ? edge.payload.properties.polarity.replaceAll('_', ' ') : '' }}</span>
              <small v-if="edge.payload.kind === 'measures' && edge.payload.properties.calibration">{{ calibrationLabel(edge.payload.properties.calibration, node.payload.properties.quantity.unit) }}</small>
            </div>
            <button type="button" class="icon-button" :aria-label="`Add observation for ${edge.destination}`" title="Add observation" @click="emit('observe', edge)"><Plus :size="15" /></button>
          </div>
          <ol v-if="edge.payload.kind === 'measures' && edge.payload.properties.observations.length" class="observation-list">
            <li v-for="observation in edge.payload.properties.observations" :key="observation.id" :class="{ superseded: replacement(edge, observation) }">
              <strong>{{ observation.value }} {{ observation.unit }}</strong>
              <span>{{ new Date(observation.observed_at).toLocaleString() }}</span>
              <small>{{ observation.source }}<template v-if="observation.measurement_standard_deviation !== null"> · σ {{ observation.measurement_standard_deviation }}</template></small>
              <small v-if="edge.payload.kind === 'measures' && edge.payload.properties.calibration" class="calibrated-reading">Normalized factor state {{ calibratedState(edge.payload.properties.calibration, observation.value)?.toFixed(3) }}</small>
              <small v-if="replacement(edge, observation)">Superseded by #{{ replacement(edge, observation)?.id }}</small>
              <small v-else-if="observation.supersedes !== null">Correction of #{{ observation.supersedes }}</small>
              <button v-if="!replacement(edge, observation)" type="button" class="icon-button observation-correct" :aria-label="`Correct observation ${observation.id} for ${edge.destination}`" title="Correct observation" @click="emit('correct', edge, observation)"><Pencil :size="13" /></button>
            </li>
          </ol>
          <p v-else class="muted">No readings recorded.</p>
        </div>
        <p v-if="!measurementEdges.length" class="muted">Create a measurement relationship to start a series.</p>
      </section>

      <section v-if="node.payload.kind === 'intervention'" class="inspector-section">
        <h3>Investment <button type="button" class="icon-button section-action" title="Add cost dimension" aria-label="Add cost dimension" @click="emit('interventionEstimate', { kind: 'cost', value: '' })"><Plus :size="14" /></button></h3>
        <div class="estimate-row">
          <div><span>Duration</span><strong>{{ node.payload.properties.duration ? formatEstimate(node.payload.properties.duration) : 'Not set' }}</strong></div>
          <button type="button" class="icon-button" aria-label="Edit duration estimate" @click="emit('interventionEstimate', { kind: 'duration' })"><Pencil :size="13" /></button>
        </div>
        <div class="estimate-row">
          <div><span>Success probability</span><strong>{{ node.payload.properties.probability_of_success ? formatEstimate(node.payload.properties.probability_of_success) : 'Not set' }}</strong></div>
          <button type="button" class="icon-button" aria-label="Edit success probability estimate" @click="emit('interventionEstimate', { kind: 'probability_of_success' })"><Pencil :size="13" /></button>
        </div>
        <div v-for="cost in node.payload.properties.costs" :key="cost.dimension" class="estimate-row">
          <div><span>{{ cost.dimension }}</span><strong>{{ formatEstimate(cost.value) }}</strong></div>
          <button type="button" class="icon-button" :aria-label="`Edit ${cost.dimension} cost estimate`" @click="emit('interventionEstimate', { kind: 'cost', value: cost.dimension })"><Pencil :size="13" /></button>
        </div>
        <p v-if="!node.payload.properties.costs.length" class="muted">No cost dimensions configured.</p>
        <div v-if="node.payload.properties.acceptance_criteria.length" class="acceptance-criteria">
          <span>Acceptance criteria</span>
          <ul><li v-for="criterion in node.payload.properties.acceptance_criteria" :key="criterion">{{ criterion }}</li></ul>
        </div>
      </section>

      <section class="inspector-section">
        <h3>Relationships <span>{{ incidentEdges.length }}</span></h3>
        <ul v-if="incidentEdges.length" class="relationship-list">
          <li v-for="edge in incidentEdges" :key="`${edge.source}-${edge.payload.kind}-${edge.destination}`">
            <button type="button" :aria-label="`Edit ${edge.payload.kind.replaceAll('_', ' ')} relationship ${edge.source} to ${edge.destination}`" @click="emit('relationship', edge)">
              <span>{{ edge.source }}</span>
              <span class="relationship-summary"><strong>{{ edge.payload.kind.replaceAll('_', ' ') }}</strong><small>{{ edgeMetadataLabel(edge) }}</small></span>
              <span>{{ edge.destination }}</span>
            </button>
          </li>
        </ul>
        <p v-else class="muted">No connected relationships.</p>
      </section>

      <details class="inspector-details">
        <summary>Identity and metadata</summary>
        <dl>
          <div><dt>Name</dt><dd>{{ node.name }}</dd></div>
          <div><dt>Revision</dt><dd>{{ node.revision }}</dd></div>
          <div v-if="node.aliases.length"><dt>Aliases</dt><dd>{{ node.aliases.join(', ') }}</dd></div>
        </dl>
        <pre v-if="Object.keys(node.metadata).length" class="metadata-view">{{ JSON.stringify(node.metadata, null, 2) }}</pre>
      </details>

      <section class="inspector-section danger-section">
        <h3>Delete node</h3>
        <p v-if="incidentEdges.length" class="muted">Delete {{ incidentEdges.length }} connected relationship{{ incidentEdges.length === 1 ? '' : 's' }} first.</p>
        <button
          type="button"
          class="danger-button"
          :disabled="incidentEdges.length > 0"
          @click="confirmDelete ? emit('delete') : (confirmDelete = true)"
        ><Trash2 :size="14" /> {{ confirmDelete ? `Confirm delete ${node.id}` : 'Delete node' }}</button>
      </section>
    </template>

    <div v-else class="empty-inspector">
      <Activity :size="24" />
      <h2>Nothing selected</h2>
      <p>Select a node in the graph or outline to inspect its typed properties.</p>
    </div>
  </aside>
</template>

<style scoped>
.inspector { min-height: 0; padding: var(--space-5) var(--space-4); overflow: auto; border-left: 1px solid var(--line); background: var(--surface); }
.inspector-header { display: flex; align-items: flex-start; gap: 10px; }
.inspector-actions { display: flex; flex-wrap: wrap; gap: 6px; margin-top: var(--space-4); }
.inspector-actions .secondary-button { min-height: 32px; padding: 0 10px; font-size: var(--text-sm); }
.readiness-panel { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 10px; margin-top: var(--space-4); padding: var(--space-3); border: 1px solid #a8bfb2; border-radius: var(--radius-md); background: #f3f8f4; color: var(--green); }
.readiness-panel[data-level='required'] { border-color: var(--danger-line); background: var(--danger-surface); color: var(--danger); }
.readiness-panel[data-level='recommended'] { border-color: #d4b171; background: #fff8e9; color: #8a5b00; }
.readiness-panel > div { min-width: 0; display: grid; gap: 8px; }
.readiness-panel strong { font-size: var(--text-sm); line-height: 1.45; }
.readiness-actions { display: flex; flex-wrap: wrap; gap: 6px; }
.readiness-actions button { min-height: 30px; display: inline-flex; align-items: center; gap: 5px; padding: 0 10px; border: 1px solid currentColor; border-radius: var(--radius-sm); background: rgba(255,255,255,.72); color: inherit; font-size: var(--text-xs); font-weight: 700; }
.kind-icon { width: 34px; height: 34px; flex: 0 0 auto; }
.inspector h2 { margin: 4px 0 0; color: var(--ink); font-size: var(--text-xl); line-height: 1.25; }
.description { margin: var(--space-4) 0; color: var(--muted); font-size: var(--text-md); line-height: 1.6; }
.inspector-section { margin-top: var(--space-5); padding-top: var(--space-4); border-top: 1px solid var(--line); }
.inspector-section h3 { display: flex; justify-content: space-between; margin: 0 0 var(--space-3); font-size: var(--text-xs); text-transform: uppercase; letter-spacing: .07em; color: #7b837e; }
.inspector-section h3 span { color: var(--muted); }
.model-section dl { margin-top: var(--space-3); }
.model-facts { display: grid; grid-template-columns: repeat(auto-fit, minmax(88px, 1fr)); gap: 6px; }
.model-facts > div { min-width: 0; display: grid; gap: 3px; padding: var(--space-2); border: 1px solid var(--line); border-radius: var(--radius-sm); background: white; }
.model-facts span { color: var(--muted); font-size: var(--text-2xs); text-transform: uppercase; letter-spacing: .04em; }
.model-facts strong { font-size: var(--text-sm); line-height: 1.35; overflow-wrap: anywhere; text-transform: capitalize; }
.model-definition { margin: var(--space-3) 0 0; padding-left: 10px; border-left: 2px solid var(--green-soft); color: var(--muted); font-size: var(--text-sm); line-height: 1.55; }
.model-empty { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: var(--space-3); border: 1px dashed #b9c2bc; border-radius: var(--radius-sm); color: var(--muted); }
.model-empty > div { display: grid; gap: 2px; }
.model-empty strong { color: var(--ink); font-size: var(--text-sm); }
.model-empty span { font-size: var(--text-xs); line-height: 1.45; }
.model-empty .secondary-button { min-height: 30px; padding: 0 10px; font-size: var(--text-xs); }
.evidence-list { display: grid; gap: 6px; margin: 0; padding: 0; list-style: none; }
.evidence-list li { display: grid; gap: 3px; padding: var(--space-3); border: 1px solid var(--line); border-radius: var(--radius-sm); background: white; }
.evidence-list strong { font-size: var(--text-sm); line-height: 1.5; }
.evidence-list span { color: var(--muted); font-size: var(--text-xs); overflow-wrap: anywhere; }
.inspector-details { margin-top: var(--space-5); padding-top: var(--space-4); border-top: 1px solid var(--line); }
.inspector-details summary { cursor: pointer; color: var(--muted); font-size: var(--text-xs); font-weight: 700; text-transform: uppercase; letter-spacing: .07em; }
.inspector-details[open] summary { margin-bottom: 12px; color: var(--ink); }
.inspector-details .metadata-view { margin-top: 12px; }
.danger-section { margin-top: var(--space-6); padding-bottom: 8px; }
.relationship-list { margin: 0; padding: 0; list-style: none; display: grid; gap: 8px; }
.relationship-list li { background: white; border: 1px solid var(--line); border-radius: var(--radius-sm); }
.relationship-list button { width: 100%; display: grid; grid-template-columns: minmax(40px, .6fr) minmax(100px, 1.4fr) minmax(40px, .6fr); gap: 8px; align-items: center; padding: var(--space-3) var(--space-2); border: 0; background: transparent; font: var(--text-sm) var(--mono); color: var(--ink); }
.relationship-list button:hover { background: var(--green-soft); }
.relationship-list button span:last-child { text-align: right; }
.relationship-list strong { color: var(--green); font-size: var(--text-xs); font-weight: 600; }
.relationship-summary { min-width: 0; display: grid; gap: 2px; text-align: center; }
.relationship-summary small { overflow: hidden; color: var(--muted); font: var(--text-xs) 'Manrope', sans-serif; text-overflow: ellipsis; white-space: nowrap; }
.metadata-view { margin: 0; padding: var(--space-3); overflow: auto; border: 1px solid var(--line); border-radius: var(--radius-sm); background: white; font: var(--text-xs)/1.6 var(--mono); color: #46504a; }
.observation-series { margin-top: 8px; padding: var(--space-3); border: 1px solid var(--line); border-radius: var(--radius-sm); background: white; }
.observation-series-header { display: flex; align-items: center; justify-content: space-between; gap: var(--space-2); }
.observation-series-header > div { display: grid; gap: 2px; }
.observation-series-header strong { font: var(--text-md) var(--mono); }
.observation-series-header span { color: var(--muted); font-size: var(--text-xs); text-transform: capitalize; }
.observation-series-header small { color: #4e6257; font-size: var(--text-2xs); line-height: 1.45; }
.observation-list { margin: var(--space-3) 0 0; padding: var(--space-3) 0 0; border-top: 1px solid var(--line); list-style: none; display: grid; gap: var(--space-3); }
.observation-list li { position: relative; display: grid; grid-template-columns: 1fr auto; gap: 2px 8px; padding-right: 32px; font-size: var(--text-sm); }
.observation-list li > span { color: var(--muted); font-size: var(--text-xs); text-align: right; }
.observation-list small { grid-column: 1 / -1; color: var(--muted); font-size: var(--text-xs); overflow-wrap: anywhere; }
.observation-list small.calibrated-reading { color: var(--green); font-weight: 700; }
.observation-list li.superseded > strong { color: var(--muted); text-decoration: line-through; }
.observation-correct { position: absolute; top: 0; right: 0; width: 26px; height: 26px; }
.section-action { width: 26px; height: 26px; margin: -6px 0; }
.equation-section { display: grid; gap: 8px; justify-items: start; }
.equation-source { margin: 0; padding: var(--space-2) var(--space-3); width: 100%; border: 1px solid var(--line); border-radius: var(--radius-sm); background: #fbfbfa; font-family: var(--mono); font-size: var(--text-sm); line-height: 1.6; white-space: pre-wrap; overflow-wrap: anywhere; }
.equation-note { margin: 0; color: var(--muted); font-size: var(--text-xs); line-height: 1.55; }
.acceptance-criteria { margin-top: var(--space-3); color: var(--muted); font-size: var(--text-sm); }
.acceptance-criteria ul { margin: 5px 0 0; padding-left: 18px; color: var(--ink); line-height: 1.6; }
.empty-inspector { min-height: 100%; display: flex; flex-direction: column; justify-content: center; align-items: center; text-align: center; color: var(--muted); }
.empty-inspector h2 { margin-top: 12px; }
.empty-inspector p { max-width: 240px; margin-top: 8px; font-size: var(--text-md); line-height: 1.55; }

@media (max-width: 760px) {
  .inspector { min-height: 220px; border-left: 0; border-top: 1px solid var(--line); }
  .empty-inspector { min-height: 220px; }
}
</style>
