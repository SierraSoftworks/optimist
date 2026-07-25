import type { Estimate, GraphEdge, GraphNode, Unit } from '../../src/api/types'

/**
 * Builders for the current wire shapes used by mocked end-to-end fixtures.
 *
 * Hand-written fixtures drifted from the API twice before: once when responses
 * replaced normalized effects, and once when factors and outcomes moved their
 * value into native quantity state. Building them here keeps every spec on one
 * definition, so the next schema change breaks one file instead of nine.
 */

export const STATE_UNIT: Unit = {}

/**
 * Builds an estimate exactly as the API returns one.
 *
 * The runtime `distribution` is `#[serde(skip)]` on the server, so a real
 * response only ever carries the Squiggle source. Fixtures must not invent it,
 * or they assert label formatting the workbench can never reach in production.
 */
export function pointEstimate(id: string, value: number, unit: Unit = STATE_UNIT): Estimate {
  return {
    id,
    revision: 0,
    source: {
      type: 'squiggle',
      definition: {
        source: `pointMass(${value})`,
        seed: 42,
        sample_count: 256,
        target_unit: unit,
      },
    },
    provenance: [],
  }
}

function identity(id: string, title: string) {
  const name = title.toLocaleLowerCase().replaceAll(' ', '_')
  return {
    id,
    revision: 0,
    name,
    normalized_name: name,
    title,
    description: '',
    aliases: [],
    metadata: {},
  }
}

function nativeState(current: Estimate | null) {
  return {
    quantity: {
      unit: 'state',
      dimension: STATE_UNIT,
      aggregation: null,
      support: { type: 'bounded' as const, lower: 0, upper: 1 },
    },
    current,
    forecast: null,
  }
}

export function factorNode(
  id: string,
  title: string,
  options: { controllable?: boolean; current?: number | null; evidence?: unknown[] } = {},
): GraphNode {
  return {
    ...identity(id, title),
    native_state: nativeState(
      options.current === null || options.current === undefined
        ? null
        : pointEstimate('A', options.current),
    ),
    payload: {
      kind: 'factor',
      properties: { controllable: options.controllable ?? false, evidence: options.evidence ?? [] },
    },
  } as unknown as GraphNode
}

export function outcomeNode(
  id: string,
  title: string,
  options: { direction?: 'maximize' | 'minimize'; current?: number | null } = {},
): GraphNode {
  return {
    ...identity(id, title),
    native_state: nativeState(
      options.current === null || options.current === undefined
        ? null
        : pointEstimate('A', options.current),
    ),
    payload: {
      kind: 'outcome',
      properties: { direction: options.direction ?? 'maximize', evidence: [] },
    },
  } as unknown as GraphNode
}

export function interventionNode(
  id: string,
  title: string,
  options: { duration?: number | null; probability?: number | null } = {},
): GraphNode {
  return {
    ...identity(id, title),
    payload: {
      kind: 'intervention',
      properties: {
        costs: [],
        duration: options.duration === undefined || options.duration === null
          ? null
          : pointEstimate('A', options.duration, { duration: 1 }),
        probability_of_success:
          options.probability === undefined || options.probability === null
            ? null
            : pointEstimate('B', options.probability),
        acceptance_criteria: [],
      },
    },
  } as unknown as GraphNode
}

/** Builds a `contributes` or `changes` relationship with a unit-aware response. */
export function causalEdge(
  source: string,
  sourceKind: string,
  destination: string,
  destinationKind: string,
  options: {
    kind?: 'contributes' | 'changes'
    response?: number
    mechanism?: string
    evidence?: string[]
  } = {},
): GraphEdge {
  const kind = options.kind ?? 'contributes'
  return {
    source,
    source_kind: sourceKind,
    destination,
    destination_kind: destinationKind,
    revision: 0,
    description: '',
    metadata: {},
    payload: {
      kind,
      properties: {
        response: {
          source_change: 1,
          source_unit: STATE_UNIT,
          destination_change: pointEstimate('A', options.response ?? 0.5),
          destination_unit: STATE_UNIT,
        },
        transience: null,
        lag: null,
        mechanism: options.mechanism ?? '',
        evidence: options.evidence ?? [],
      },
    },
  } as unknown as GraphEdge
}
