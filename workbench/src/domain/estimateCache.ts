import type {
  Estimate,
  GraphNode,
  InterventionEstimateSlot,
  StateEstimateSlot,
} from '../api/types'

export function setStateEstimate(
  node: GraphNode,
  slot: StateEstimateSlot,
  estimate: Estimate,
): GraphNode {
  if (node.native_state) {
    return {
      ...node,
      native_state: {
        ...node.native_state,
        [slot === 'current' ? 'current' : 'forecast']: estimate,
      },
    }
  }
  if (node.payload.kind === 'metric') {
    return {
      ...node,
      payload: {
        ...node.payload,
        properties: { ...node.payload.properties, current: estimate },
      },
    }
  }
  if (node.payload.kind === 'factor') {
    return {
      ...node,
      payload: {
        ...node.payload,
        properties: { ...node.payload.properties, [slot]: estimate },
      },
    }
  }
  if (node.payload.kind === 'outcome') {
    return {
      ...node,
      payload: {
        ...node.payload,
        properties: { ...node.payload.properties, [slot]: estimate },
      },
    }
  }
  return node
}

export function setInterventionEstimate(
  node: GraphNode,
  slot: InterventionEstimateSlot,
  estimate: Estimate,
): GraphNode {
  if (node.payload.kind !== 'intervention') return node
  if (slot.kind === 'cost') {
    const costs = node.payload.properties.costs.some((cost) => cost.dimension === slot.value)
      ? node.payload.properties.costs.map((cost) =>
          cost.dimension === slot.value ? { ...cost, value: estimate } : cost,
        )
      : [...node.payload.properties.costs, { dimension: slot.value, value: estimate }]
    return {
      ...node,
      payload: {
        ...node.payload,
        properties: { ...node.payload.properties, costs },
      },
    }
  }
  return {
    ...node,
    payload: {
      ...node.payload,
      properties: {
        ...node.payload.properties,
        [slot.kind]: estimate,
      },
    },
  }
}
