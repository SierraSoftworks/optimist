import type { MeasurementCalibration, Observation } from '../api/types'

export function calibratedState(calibration: MeasurementCalibration, reading: number) {
  if (!Number.isFinite(reading)) return null
  const state = calibration.type === 'linear'
    ? (reading - calibration.state_zero) / (calibration.state_one - calibration.state_zero)
    : reading < calibration.ideal_lower
      ? (reading - calibration.outer_lower) / (calibration.ideal_lower - calibration.outer_lower)
      : reading > calibration.ideal_upper
        ? (calibration.outer_upper - reading) / (calibration.outer_upper - calibration.ideal_upper)
        : 1
  return Math.max(0, Math.min(1, state))
}

export function calibrationLabel(calibration: MeasurementCalibration, unit: string) {
  if (calibration.type === 'linear') {
    return `${calibration.state_zero} ${unit} → state 0 · ${calibration.state_one} ${unit} → state 1`
  }
  return `${calibration.outer_lower}–${calibration.ideal_lower} ${unit} ramps to state 1 · ${calibration.ideal_lower}–${calibration.ideal_upper} ${unit} is ideal · ${calibration.ideal_upper}–${calibration.outer_upper} ${unit} ramps to state 0`
}

export function currentObservations(observations: Observation[]) {
  const superseded = new Set(
    observations.flatMap((observation) => observation.supersedes === null ? [] : [observation.supersedes]),
  )
  return observations.filter((observation) => !superseded.has(observation.id))
}

export function latestObservation(observations: Observation[]) {
  return currentObservations(observations).reduce<Observation | null>((latest, observation) => {
    if (!latest) return observation
    return Date.parse(observation.observed_at) >= Date.parse(latest.observed_at) ? observation : latest
  }, null)
}