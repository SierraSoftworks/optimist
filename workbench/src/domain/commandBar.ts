import type {
  CreateEdgeInput,
  CreateNodeInput,
  GraphEdge,
  GraphNode,
  NodeKind,
} from '../api/types'
import type { WorkbenchMode } from '../stores/workbench'
import { edgePayload, endpointsAreValid, nodeUnit } from './edgeAuthoring'
import { normalizedEdgeKind, resolveCommandNode, tokenizeCommand } from './commandBarSyntax'
import { parseUnitExpression } from './unitExpression'
export { commandSuggestions, type CommandSuggestion } from './commandBarSuggestions'

export type WorkbenchCommand =
  | { type: 'create_node'; input: CreateNodeInput }
  | { type: 'create_edge'; input: CreateEdgeInput }
  | { type: 'select_node'; node: GraphNode }
  | { type: 'set_mode'; mode: WorkbenchMode }

export interface CommandDiagnostic {
  severity: 'hint' | 'error'
  message: string
}

export interface CommandResult {
  command: WorkbenchCommand | null
  diagnostic: CommandDiagnostic
}

const modes: WorkbenchMode[] = ['explore', 'impediments', 'feedback', 'optimize']
const nodeKinds: NodeKind[] = ['factor', 'outcome', 'metric', 'intervention']

function nodeInput(kind: NodeKind, title: string, option?: string): CreateNodeInput | null {
  const name = title
    .trim()
    .toLocaleLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_|_$/g, '')
  if (!name) return null
  if (kind === 'metric') {
    if (!option) return null
    let dimension
    try {
      dimension = parseUnitExpression(option)
    } catch {
      return null
    }
    return {
      name,
      title,
      payload: {
        kind,
        properties: {
          quantity: { unit: option, dimension, aggregation: null, support: { type: 'real' } },
          current: null,
        },
      },
    }
  }
  if (kind === 'intervention') {
    return {
      name,
      title,
      payload: {
        kind,
        properties: {
          costs: [],
          duration: null,
          probability_of_success: null,
          acceptance_criteria: [],
        },
      },
    }
  }
  if (kind === 'outcome') {
    const direction = ['maximize', 'minimize', 'target_range'].includes(option ?? '')
      ? option as 'maximize' | 'minimize' | 'target_range'
      : 'maximize'
    return {
      name,
      title,
      payload: { kind, properties: { direction, evidence: [] } },
    }
  }
  return {
    name,
    title,
    payload: {
      kind,
      properties: {
        controllable: option === 'controllable',
        evidence: [],
      },
    },
  }
}

export function parseCommand(input: string, nodes: GraphNode[], edges: GraphEdge[]): CommandResult {
  const { tokens, incompleteQuote } = tokenizeCommand(input)
  if (!tokens.length) return hint('Choose a command.')
  if (incompleteQuote) return error('Close the quoted value before applying.')
  const [verb, ...args] = tokens
  if (verb === 'add') {
    const kind = args[0]?.toLocaleLowerCase() as NodeKind
    if (!nodeKinds.includes(kind)) return error('Choose factor, outcome, metric, or intervention.')
    if (!args[1]) return hint(`Add a title for the ${kind}.`)
    const input = nodeInput(kind, args[1], args[2])
    if (!input) return error(kind === 'metric' ? 'Metrics require a unit.' : 'The node title is invalid.')
    return ready({ type: 'create_node', input })
  }
  if (verb === 'connect') {
    const source = args[0] ? resolveCommandNode(args[0], nodes) : null
    if (!source) return hint('Choose one source node by ID, name, or exact title.')
    const kind = args[1] ? normalizedEdgeKind(args[1]) : null
    if (!kind) return hint('Choose a relationship kind.')
    const destination = args[2] ? resolveCommandNode(args[2], nodes) : null
    if (!destination) return hint('Choose one destination node by ID, name, or exact title.')
    if (!endpointsAreValid(kind, source.payload.kind, destination.payload.kind)) {
      return error(`${kind.replaceAll('_', ' ')} is not valid between these node kinds.`)
    }
    if (edges.some((edge) =>
      edge.source === source.id && edge.destination === destination.id && edge.payload.kind === kind,
    )) return error('That relationship already exists.')
    const causal = kind === 'contributes' || kind === 'changes'
    const sourceChange = causal ? Number(args[3]) : undefined
    const destinationChange = causal ? Number(args[4]) : undefined
    if (causal && (args[3] === undefined || args[4] === undefined)) {
      return hint('Provide source change and destination change for this response.')
    }
    if (causal && (!Number.isFinite(sourceChange) || sourceChange === 0 || !Number.isFinite(destinationChange))) {
      return error('Response changes must be finite and source change cannot be zero.')
    }
    if (causal && (!nodeUnit(source) || !nodeUnit(destination))) {
      return error('Both causal endpoints require canonical quantity dimensions.')
    }
    const effect = kind === 'blocks' ? (args[3] === undefined ? 0.5 : Number(args[3])) : 0
    if (kind === 'blocks' && (!Number.isFinite(effect) || effect < 0 || effect > 1)) {
      return error('Blocking degree must be between 0 and 1.')
    }
    return ready({
      type: 'create_edge',
      input: {
        source: source.id,
        destination: destination.id,
        payload: edgePayload({
          kind,
          effect,
          lag: null,
          mechanism: '',
          evidence: '',
          polarity: 'higher_is_better',
          hard: true,
          threshold: null,
          source,
          destination,
          sourceChange,
          destinationChange,
        }),
      },
    })
  }
  if (verb === 'select') {
    const node = args[0] ? resolveCommandNode(args[0], nodes) : null
    return node ? ready({ type: 'select_node', node }) : hint('Choose one node by ID, name, or exact title.')
  }
  if (verb === 'mode') {
    const mode = args[0]?.toLocaleLowerCase() as WorkbenchMode
    return modes.includes(mode) ? ready({ type: 'set_mode', mode }) : hint('Choose explore, impediments, feedback, or optimize.')
  }
  return error(`Unknown command ${verb}.`)
}

export function commandPreview(command: WorkbenchCommand): Array<[string, string]> {
  if (command.type === 'create_node') {
    const payload = command.input.payload
    const setup = payload.kind === 'metric'
      ? `Unit ${payload.properties.quantity.unit}`
      : payload.kind === 'intervention'
        ? 'Duration + success need Squiggle estimates'
        : 'State quantity + current estimate required'
    return [['Action', 'Create node'], ['Kind', payload.kind], ['Title', command.input.title], ['Setup', setup]]
  }
  if (command.type === 'create_edge') {
    const payload = command.input.payload
    const preview: Array<[string, string]> = [
      ['Action', 'Create relationship'],
      ['Route', `${command.input.source} → ${command.input.destination}`],
      ['Kind', payload.kind.replaceAll('_', ' ')],
    ]
    if (payload.kind === 'contributes' || payload.kind === 'changes') {
      preview.push(['Source change', String(payload.properties.response.source_change)])
      preview.push(['Destination change', String(payload.properties.response.destination_change.distribution.value)])
    }
    if (payload.kind === 'blocks') {
      preview.push(['Degree', String(payload.properties.degree.distribution.value)])
    }
    return preview
  }
  if (command.type === 'select_node') return [['Action', 'Inspect node'], ['Node', `${command.node.title} · ${command.node.id}`]]
  return [['Action', 'Switch mode'], ['Mode', command.mode]]
}

function hint(message: string): CommandResult {
  return { command: null, diagnostic: { severity: 'hint', message } }
}

function error(message: string): CommandResult {
  return { command: null, diagnostic: { severity: 'error', message } }
}

function ready(command: WorkbenchCommand): CommandResult {
  return { command, diagnostic: { severity: 'hint', message: 'Ready to apply.' } }
}