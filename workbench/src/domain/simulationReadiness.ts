import type { GraphNode } from '../api/types'

export type ReadinessSeverity = 'required' | 'recommended'

export interface ReadinessIssue {
  key: string
  label: string
  severity: ReadinessSeverity
}

export interface NodeReadiness {
  level: 'ready' | ReadinessSeverity
  issues: ReadinessIssue[]
}

export function simulationReadiness(node: GraphNode): NodeReadiness {
  const issues: ReadinessIssue[] = []
  if (node.payload.kind === 'outcome' || node.payload.kind === 'factor') {
    if (!node.payload.properties.current) {
      issues.push({
        key: 'current_state',
        label: 'Current state estimate',
        severity: 'required',
      })
    }
  }
  if (node.payload.kind === 'intervention') {
    if (!node.payload.properties.probability_of_success) {
      issues.push({
        key: 'success_probability',
        label: 'Success probability',
        severity: 'recommended',
      })
    }
    if (!node.payload.properties.duration) {
      issues.push({
        key: 'duration',
        label: 'Duration estimate',
        severity: 'recommended',
      })
    }
  }
  return {
    level: issues.some((issue) => issue.severity === 'required')
      ? 'required'
      : issues.length
        ? 'recommended'
        : 'ready',
    issues,
  }
}

export function readinessLabel(readiness: NodeReadiness) {
  if (readiness.level === 'ready') return 'Simulation ready'
  const prefix = readiness.level === 'required' ? 'Simulation blocked' : 'Setup recommended'
  return `${prefix}: ${readiness.issues.map((issue) => issue.label).join(', ')}`
}