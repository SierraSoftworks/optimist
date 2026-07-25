import type {
  EffectProfileInput,
  EffectReleaseInput,
  SquiggleEstimateDefinition,
  Unit,
} from '../api/types'

const PERIOD_UNIT: Unit = { duration: 1 }

/** How a shaped effect subsides once its hold window ends. */
export type ReleaseShape = 'immediate' | 'linear' | 'exponential'

/**
 * Form state behind the effect profile editor.
 *
 * Durations are whole planning periods here because the editor authors point
 * masses. The wire format keeps full Squiggle programs, so uncertain schedules
 * remain expressible without changing this shape.
 *
 * `reboundMagnitude` is a dimensionless multiplier applied while the rebound
 * runs, so `1` is no rebound at all and `1.25` returns a quarter more than
 * baseline for its window.
 */
export interface EffectProfileForm {
  enabled: boolean
  ramp: number
  hold: number
  release: ReleaseShape
  releaseSpan: number
  reboundEnabled: boolean
  reboundMagnitude: number
  reboundHold: number
}

export function emptyEffectProfileForm(): EffectProfileForm {
  return {
    enabled: false,
    ramp: 0,
    hold: 2,
    release: 'immediate',
    releaseSpan: 1,
    reboundEnabled: false,
    reboundMagnitude: 1.25,
    reboundHold: 1,
  }
}

export function periodsDefinition(periods: number): SquiggleEstimateDefinition {
  return {
    source: `pointMass(${periods})`,
    seed: 42,
    sample_count: 256,
    target_unit: PERIOD_UNIT,
  }
}

function releaseInput(form: EffectProfileForm): EffectReleaseInput {
  if (form.release === 'linear') {
    return { type: 'linear', over: periodsDefinition(form.releaseSpan) }
  }
  if (form.release === 'exponential') {
    return { type: 'exponential', half_life: periodsDefinition(form.releaseSpan) }
  }
  return { type: 'immediate' }
}

/**
 * Builds the wire profile, or `null` for a permanent effect.
 *
 * A profile is only transient when it departs from a permanent step, so a form
 * with no ramp, no hold, and no rebound restores the unshaped effect rather than
 * sending a shape the server would reject.
 */
export function effectProfileInput(form: EffectProfileForm): EffectProfileInput | null {
  if (!form.enabled) return null
  const ramp = form.ramp > 0 ? periodsDefinition(form.ramp) : null
  const hold = periodsDefinition(form.hold)
  if (!form.reboundEnabled) {
    return { ramp, hold, release: releaseInput(form), aftereffect: null }
  }
  return {
    ramp,
    hold,
    release: releaseInput(form),
    aftereffect: {
      magnitude: {
        source: `pointMass(${form.reboundMagnitude})`,
        seed: 42,
        sample_count: 256,
        target_unit: {},
      },
      hold: periodsDefinition(form.reboundHold),
      release: { type: 'immediate' },
    },
  }
}

export function effectProfileValid(form: EffectProfileForm): boolean {
  if (!form.enabled) return true
  if (!Number.isFinite(form.ramp) || form.ramp < 0) return false
  if (!Number.isFinite(form.hold) || form.hold < 0) return false
  if (form.release !== 'immediate' && !(form.releaseSpan > 0)) return false
  if (form.reboundEnabled) {
    if (!Number.isFinite(form.reboundMagnitude)) return false
    if (!Number.isFinite(form.reboundHold) || form.reboundHold < 0) return false
  }
  return form.ramp > 0 || form.hold > 0 || form.reboundEnabled
}

function remaining(form: EffectProfileForm, elapsed: number): number {
  const periods = elapsed + 1
  if (form.release === 'linear') {
    return Math.max(0, 1 - periods / (form.releaseSpan + 1))
  }
  if (form.release === 'exponential') {
    return form.releaseSpan > 0 ? 2 ** (-periods / form.releaseSpan) : 0
  }
  return 0
}

/**
 * Mirrors the server's activation kernels so the editor can preview a shape
 * before it is saved.
 *
 * Both series are the deterministic point-mass case of the sampled profile:
 * `a(e) = (e + 1) / (r + 1)` while ramping, `1` while held, and the release
 * kernel afterwards. Uncertain durations widen these curves on the server, so
 * the preview shows the median schedule rather than a guarantee.
 */
export function activationSeries(
  form: EffectProfileForm,
  periods: number,
): { activation: number[]; rebound: number[] } {
  const activation: number[] = []
  const rebound: number[] = []
  for (let elapsed = 0; elapsed < periods; elapsed += 1) {
    if (!form.enabled) {
      activation.push(1)
      rebound.push(0)
      continue
    }
    const released = elapsed - form.ramp - form.hold
    if (elapsed < form.ramp) {
      activation.push((elapsed + 1) / (form.ramp + 1))
    } else if (released < 0) {
      activation.push(1)
    } else {
      activation.push(remaining(form, released))
    }
    if (!form.reboundEnabled || released < 0) {
      rebound.push(0)
    } else {
      rebound.push(released < form.reboundHold ? 1 : 0)
    }
  }
  return { activation, rebound }
}
