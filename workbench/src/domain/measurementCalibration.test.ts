import { describe, expect, it } from 'vitest'
import type { Observation } from '../api/types'
import {
  calibratedState,
  calibrationLabel,
  currentObservations,
  latestObservation,
} from './measurementCalibration'

describe('measurement calibration', () => {
  it('maps linear readings in either direction and clamps outer values', () => {
    expect(calibratedState({ type: 'linear', state_zero: 10, state_one: 30 }, 20)).toBe(0.5)
    expect(calibratedState({ type: 'linear', state_zero: 30, state_one: 10 }, 20)).toBe(0.5)
    expect(calibratedState({ type: 'linear', state_zero: 30, state_one: 10 }, 5)).toBe(1)
  })

  it('maps target ranges through both ramps and the ideal plateau', () => {
    const calibration = {
      type: 'target_range' as const,
      outer_lower: 50,
      ideal_lower: 80,
      ideal_upper: 120,
      outer_upper: 150,
    }
    expect(calibratedState(calibration, 65)).toBe(0.5)
    expect(calibratedState(calibration, 100)).toBe(1)
    expect(calibratedState(calibration, 135)).toBe(0.5)
    expect(calibrationLabel(calibration, 'ms')).toContain('80–120 ms is ideal')
  })

  it('uses corrections instead of superseded readings when choosing latest evidence', () => {
    const observations = [
      observation(0, 20, '2026-01-01T00:00:00Z', null),
      observation(1, 18, '2026-01-01T00:00:00Z', 0),
      observation(2, 15, '2026-02-01T00:00:00Z', null),
    ]
    expect(currentObservations(observations).map(({ id }) => id)).toEqual([1, 2])
    expect(latestObservation(observations)?.id).toBe(2)
  })
})

function observation(id: number, value: number, observed_at: string, supersedes: number | null): Observation {
  return {
    id, value, observed_at, supersedes,
    revision: 0, unit: 'days', source: 'test', measurement_standard_deviation: null,
  }
}