<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Activity, AlertTriangle, CheckCircle2, Gauge, Goal, Pencil, Plus, Sigma, Trash2, Wrench } from '@lucide/vue'
import type {
  Distribution,
  Evidence,
  GraphEdge,
  GraphNode,
  InterventionEstimateSlot,
  Observation,
} from '../api/types'
import { calibratedState, calibrationLabel } from '../domain/measurementCalibration'
import { readinessLabel, simulationReadiness } from '../domain/simulationReadiness'
import { edgeMetadataLabel } from '../domain/edgePresentation'
import SquiggleChartIsland from './SquiggleChartIsland.vue'

const props = defineProps<{ node: GraphNode | null; edges: GraphEdge[] }>()
const emit = defineEmits<{
  edit: []
  estimate: []
  relationship: [edge: GraphEdge]
  observe: [edge: GraphEdge]
  correct: [edge: GraphEdge, observation: Observation]
  interventionEstimate: [slot: InterventionEstimateSlot]
  evidence: [evidence: Evidence | null]
  delete: []
}>()
const confirmDelete = ref(false)
const readiness = computed(() => props.node ? simulationReadiness(props.node) : null)
const distributionCharts = computed(() => {
  const node = props.node
  if (!node) return []
  const estimates: Array<[string, import('../api/types').Estimate | null | undefined]> = []
  if (node.payload.kind === 'factor' || node.payload.kind === 'outcome') {
    estimates.push(['Current state', node.payload.properties.current], ['Desired state', node.payload.properties.desired])
  } else if (node.payload.kind === 'metric') {
    estimates.push(['Current quantity', node.payload.properties.current])
  } else {
    estimates.push(
      ['Duration', node.payload.properties.duration],
      ['Success probability', node.payload.properties.probability_of_success],
      ...node.payload.properties.costs.map((cost): [string, import('../api/types').Estimate] => [`${cost.dimension} cost`, cost.value]),
    )
  }
  return estimates.flatMap(([label, estimate]) => estimate?.source?.type === 'squiggle'
    ? [{ label, source: estimate.source.definition.source }]
    : [])
})

const incidentEdges = computed(() =>
  props.node
    ? props.edges.filter(
        (edge) => edge.source === props.node?.id || edge.destination === props.node?.id,
      )
    : [],
)
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

function distribution(node: GraphNode, slot: 'current' | 'desired') {
  if (node.payload.kind !== 'outcome' && node.payload.kind !== 'factor') return null
  return node.payload.properties[slot]?.distribution ?? null
}

function distributionLabel(node: GraphNode, slot: 'current' | 'desired') {
  const value = distribution(node, slot)
  if (!value) return 'Not set'
  return formatDistribution(value)
}

function sourceLabel(node: GraphNode, slot: 'current' | 'desired') {
  if (node.payload.kind !== 'outcome' && node.payload.kind !== 'factor') return null
  const estimate = node.payload.properties[slot]
  if (estimate?.source?.type === 'squiggle') {
    return `${estimate.source.definition.source} · ${estimate.source.assessment.family} · ${estimate.source.assessment.sample_count.toLocaleString()} samples`
  }
  if (estimate?.source?.type === 'fermi') {
    return `Legacy Fermi · ${estimate.source.definition.equation}`
  }
  return null
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

function estimateSourceLabel(estimate: import('../api/types').Estimate | null) {
  if (estimate?.source?.type === 'squiggle') return `Squiggle · ${estimate.source.definition.source}`
  if (estimate?.source?.type === 'fermi') return `Legacy Fermi · ${estimate.source.definition.equation}`
  return null
}

function provenance(node: GraphNode, slot: 'current' | 'desired') {
  if (node.payload.kind !== 'outcome' && node.payload.kind !== 'factor') return []
  return node.payload.properties[slot]?.provenance ?? []
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
          v-if="node.payload.kind === 'outcome' || node.payload.kind === 'factor' || node.payload.kind === 'metric'"
          type="button"
          class="secondary-button"
          @click="emit('estimate')"
        ><Sigma :size="14" /> Estimate</button>
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
              @click="issue.key === 'current_state' ? emit('estimate') : emit('interventionEstimate', { kind: issue.key === 'duration' ? 'duration' : 'probability_of_success' })"
            >{{ issue.label }} <Pencil :size="11" /></button>
          </div>
        </div>
      </section>

      <p v-if="node.description" class="description">{{ node.description }}</p>
      <p v-else class="muted">No description has been added.</p>

      <section class="inspector-section">
        <h3>Identity</h3>
        <dl>
          <div><dt>Name</dt><dd>{{ node.name }}</dd></div>
          <div><dt>Revision</dt><dd>{{ node.revision }}</dd></div>
          <div v-if="node.aliases.length"><dt>Aliases</dt><dd>{{ node.aliases.join(', ') }}</dd></div>
        </dl>
      </section>

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

      <section v-if="node.payload.kind === 'outcome' || node.payload.kind === 'factor'" class="inspector-section">
        <h3>State estimates</h3>
        <dl>
          <div><dt>Current</dt><dd>{{ distributionLabel(node, 'current') }}</dd></div>
          <div v-if="sourceLabel(node, 'current')"><dt>Current model</dt><dd>{{ sourceLabel(node, 'current') }}</dd></div>
          <div v-if="provenance(node, 'current').length"><dt>Current source</dt><dd>{{ provenance(node, 'current').join('; ') }}</dd></div>
          <div><dt>Desired</dt><dd>{{ distributionLabel(node, 'desired') }}</dd></div>
          <div v-if="sourceLabel(node, 'desired')"><dt>Desired model</dt><dd>{{ sourceLabel(node, 'desired') }}</dd></div>
          <div v-if="provenance(node, 'desired').length"><dt>Desired source</dt><dd>{{ provenance(node, 'desired').join('; ') }}</dd></div>
          <div v-if="node.payload.kind === 'factor'"><dt>Controllable</dt><dd>{{ node.payload.properties.controllable ? 'Yes' : 'No' }}</dd></div>
          <div v-if="node.payload.kind === 'outcome'"><dt>Direction</dt><dd>{{ node.payload.properties.direction }}</dd></div>
        </dl>
      </section>

      <section v-if="node.payload.kind === 'outcome' || node.payload.kind === 'factor'" class="inspector-section">
        <h3>Evidence <button type="button" class="icon-button section-action" title="Add evidence" aria-label="Add evidence" @click="emit('evidence', null)"><Plus :size="14" /></button></h3>
        <div v-for="item in node.payload.properties.evidence" :key="item.id" class="evidence-row">
          <button type="button" :aria-label="`Edit evidence ${item.id}`" @click="emit('evidence', item)">
            <strong>{{ item.summary }}</strong>
            <span>{{ item.source ?? 'No source' }} · r{{ item.revision }}</span>
          </button>
        </div>
        <p v-if="!node.payload.properties.evidence.length" class="muted">No qualitative evidence recorded.</p>
      </section>

      <section v-if="Object.keys(node.metadata).length" class="inspector-section">
        <h3>Metadata</h3>
        <pre class="metadata-view">{{ JSON.stringify(node.metadata, null, 2) }}</pre>
      </section>

      <section v-if="node.payload.kind === 'metric'" class="inspector-section">
        <h3>Native quantity</h3>
        <dl>
          <div><dt>Unit</dt><dd>{{ node.payload.properties.unit }}</dd></div>
          <div><dt>Aggregation</dt><dd>{{ node.payload.properties.aggregation ?? 'Not set' }}</dd></div>
          <div><dt>Support</dt><dd>{{ node.payload.properties.support?.type.replaceAll('_', ' ') ?? 'real' }}</dd></div>
          <div v-if="node.payload.properties.support?.type === 'bounded'"><dt>Bounds</dt><dd>{{ node.payload.properties.support.lower }}–{{ node.payload.properties.support.upper }}</dd></div>
          <div><dt>Current estimate</dt><dd>{{ node.payload.properties.current ? formatDistribution(node.payload.properties.current.distribution) : 'Not set' }}</dd></div>
          <div v-if="node.payload.properties.operational_definition"><dt>Definition</dt><dd>{{ node.payload.properties.operational_definition }}</dd></div>
          <div v-if="node.payload.properties.reference_time"><dt>Reference time</dt><dd>{{ node.payload.properties.reference_time }}</dd></div>
          <div v-if="node.payload.properties.resolution_source"><dt>Resolution source</dt><dd>{{ node.payload.properties.resolution_source }}</dd></div>
        </dl>
      </section>

      <section v-if="node.payload.kind === 'metric'" class="inspector-section">
        <h3>Observation series <span>{{ measurementEdges.length }}</span></h3>
        <div v-for="edge in measurementEdges" :key="edge.destination" class="observation-series">
          <div class="observation-series-header">
            <div>
              <strong>{{ edge.destination }}</strong>
              <span>{{ edge.payload.kind === 'measures' ? edge.payload.properties.polarity.replaceAll('_', ' ') : '' }}</span>
              <small v-if="edge.payload.kind === 'measures' && edge.payload.properties.calibration">{{ calibrationLabel(edge.payload.properties.calibration, node.payload.kind === 'metric' ? node.payload.properties.unit : '') }}</small>
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
          <div><span>Duration</span><strong>{{ node.payload.properties.duration ? formatDistribution(node.payload.properties.duration.distribution) : 'Not set' }}</strong><small v-if="estimateSourceLabel(node.payload.properties.duration)">{{ estimateSourceLabel(node.payload.properties.duration) }}</small></div>
          <button type="button" class="icon-button" aria-label="Edit duration estimate" @click="emit('interventionEstimate', { kind: 'duration' })"><Pencil :size="13" /></button>
        </div>
        <div class="estimate-row">
          <div><span>Success probability</span><strong>{{ node.payload.properties.probability_of_success ? formatDistribution(node.payload.properties.probability_of_success.distribution) : 'Not set' }}</strong><small v-if="estimateSourceLabel(node.payload.properties.probability_of_success)">{{ estimateSourceLabel(node.payload.properties.probability_of_success) }}</small></div>
          <button type="button" class="icon-button" aria-label="Edit success probability estimate" @click="emit('interventionEstimate', { kind: 'probability_of_success' })"><Pencil :size="13" /></button>
        </div>
        <div v-for="cost in node.payload.properties.costs" :key="cost.dimension" class="estimate-row">
          <div><span>{{ cost.dimension }}</span><strong>{{ formatDistribution(cost.value.distribution) }}</strong><small v-if="estimateSourceLabel(cost.value)">{{ estimateSourceLabel(cost.value) }}</small></div>
          <button type="button" class="icon-button" :aria-label="`Edit ${cost.dimension} cost estimate`" @click="emit('interventionEstimate', { kind: 'cost', value: cost.dimension })"><Pencil :size="13" /></button>
        </div>
        <p v-if="!node.payload.properties.costs.length" class="muted">No cost dimensions configured.</p>
        <div v-if="node.payload.properties.acceptance_criteria.length" class="acceptance-criteria">
          <span>Acceptance criteria</span>
          <ul><li v-for="criterion in node.payload.properties.acceptance_criteria" :key="criterion">{{ criterion }}</li></ul>
        </div>
      </section>

      <section v-if="distributionCharts.length" class="inspector-section">
        <h3>Distribution models <span>{{ distributionCharts.length }}</span></h3>
        <div class="distribution-charts">
          <div v-for="chart in distributionCharts" :key="chart.label">
            <strong>{{ chart.label }}</strong>
            <SquiggleChartIsland :code="chart.source" :label="`${chart.label} distribution`" :height="150" />
          </div>
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
    </template>

    <div v-else class="empty-inspector">
      <Activity :size="24" />
      <h2>Nothing selected</h2>
      <p>Select a node in the graph or outline to inspect its typed properties.</p>
    </div>
  </aside>
</template>

<style scoped>
.inspector { min-height: 0; padding: 18px; overflow: auto; border-left: 1px solid var(--line); background: var(--surface); }
.inspector-header { display: flex; align-items: flex-start; gap: 10px; }
.inspector-actions { display: flex; gap: 6px; margin-top: 14px; }
.inspector-actions .secondary-button { min-height: 30px; padding: 0 9px; }
.readiness-panel { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 9px; margin-top: 14px; padding: 10px; border: 1px solid #a8bfb2; border-radius: 6px; background: #f3f8f4; color: var(--green); }
.readiness-panel[data-level='required'] { border-color: #d8a098; background: #fff8f6; color: #9a3e31; }
.readiness-panel[data-level='recommended'] { border-color: #d4b171; background: #fff8e9; color: #8a5b00; }
.readiness-panel > div { min-width: 0; display: grid; gap: 7px; }
.readiness-panel strong { font-size: 10px; line-height: 1.4; }
.readiness-actions { display: flex; flex-wrap: wrap; gap: 5px; }
.readiness-actions button { min-height: 26px; display: inline-flex; align-items: center; gap: 5px; padding: 0 7px; border: 1px solid currentColor; border-radius: 4px; background: rgba(255,255,255,.72); color: inherit; font-size: 8px; font-weight: 700; }
.kind-icon { width: 34px; height: 34px; flex: 0 0 auto; }
.inspector h2 { margin: 3px 0 0; color: var(--ink); font-size: 16px; line-height: 1.25; }
.description { margin: 16px 0; color: var(--muted); font-size: 11px; line-height: 1.55; }
.inspector-section { margin-top: 20px; padding-top: 15px; border-top: 1px solid var(--line); }
.inspector-section h3 { display: flex; justify-content: space-between; margin: 0 0 9px; font-size: 11px; text-transform: uppercase; letter-spacing: .06em; }
.inspector-section h3 span { color: var(--muted); }
.relationship-list { margin: 0; padding: 0; list-style: none; display: grid; gap: 5px; }
.relationship-list li { background: white; border: 1px solid var(--line); border-radius: 5px; }
.relationship-list button { width: 100%; display: grid; grid-template-columns: minmax(34px, .6fr) minmax(92px, 1.4fr) minmax(34px, .6fr); gap: 6px; align-items: center; padding: 7px; border: 0; background: transparent; font: 9px 'IBM Plex Mono', monospace; color: var(--ink); }
.relationship-list button:hover { background: var(--green-soft); }
.relationship-list button span:last-child { text-align: right; }
.relationship-list strong { color: var(--green); font-size: 8px; font-weight: 500; }
.relationship-summary { min-width: 0; display: grid; gap: 2px; text-align: center; }
.relationship-summary small { overflow: hidden; color: var(--muted); font: 7px 'Manrope', sans-serif; text-overflow: ellipsis; white-space: nowrap; }
.metadata-view { margin: 0; padding: 9px; overflow: auto; border: 1px solid var(--line); border-radius: 5px; background: white; font: 9px/1.5 'IBM Plex Mono', monospace; color: #46504a; }
.observation-series { margin-top: 8px; padding: 9px; border: 1px solid var(--line); border-radius: 5px; background: white; }
.observation-series-header { display: flex; align-items: center; justify-content: space-between; }
.observation-series-header > div { display: grid; gap: 2px; }
.observation-series-header strong { font: 11px 'IBM Plex Mono', monospace; }
.observation-series-header span { color: var(--muted); font-size: 9px; text-transform: capitalize; }
.observation-series-header small { color: #4e6257; font-size: 8px; line-height: 1.4; }
.observation-list { margin: 9px 0 0; padding: 9px 0 0; border-top: 1px solid var(--line); list-style: none; display: grid; gap: 9px; }
.observation-list li { position: relative; display: grid; grid-template-columns: 1fr auto; gap: 2px 8px; padding-right: 30px; font-size: 10px; }
.observation-list li > span { color: var(--muted); font-size: 9px; text-align: right; }
.observation-list small { grid-column: 1 / -1; color: var(--muted); font-size: 9px; overflow-wrap: anywhere; }
.observation-list small.calibrated-reading { color: var(--green); font-weight: 700; }
.observation-list li.superseded > strong { color: var(--muted); text-decoration: line-through; }
.observation-correct { position: absolute; top: 0; right: 0; width: 24px; height: 24px; }
.section-action { width: 24px; height: 24px; margin: -6px 0; }
.acceptance-criteria { margin-top: 12px; color: var(--muted); font-size: 9px; }
.acceptance-criteria ul { margin: 5px 0 0; padding-left: 17px; color: var(--ink); line-height: 1.55; }
.distribution-charts { display: grid; gap: 10px; }
.distribution-charts > div { display: grid; gap: 5px; }
.distribution-charts > div > strong { font-size: 9px; }
.evidence-row { margin-top: 6px; border: 1px solid var(--line); border-radius: 5px; background: white; }
.evidence-row button { width: 100%; display: grid; gap: 3px; padding: 8px; border: 0; background: transparent; text-align: left; }
.evidence-row button:hover { background: var(--green-soft); }
.evidence-row strong { font-size: 10px; line-height: 1.45; }
.evidence-row span { color: var(--muted); font-size: 9px; overflow-wrap: anywhere; }
.empty-inspector { min-height: 100%; display: flex; flex-direction: column; justify-content: center; align-items: center; text-align: center; color: var(--muted); }
.empty-inspector h2 { margin-top: 12px; }
.empty-inspector p { max-width: 210px; margin-top: 7px; font-size: 11px; line-height: 1.5; }

@media (max-width: 760px) {
  .inspector { min-height: 220px; border-left: 0; border-top: 1px solid var(--line); }
  .empty-inspector { min-height: 220px; }
}
</style>
