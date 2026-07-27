import { formatSiNumber } from './humanNumber'

/**
 * Reading a quantity in the units it was declared in.
 *
 * A chart axis labelled `0.87` when the quantity is a success rate is asking the
 * reader to do a conversion the model already knows how to do. The unit comes
 * from the component type's channel declaration, so the interface can present
 * the number the way the person who declared it thinks about it.
 */
export interface Scale {
  /** Multiply a raw value by this before showing it. */
  factor: number
  /** What to write after the number, empty where the unit is implied by it. */
  suffix: string
}

/**
 * Chooses how to present a quantity.
 *
 * The only rescaling done is dimensionless-to-percent, and only when every value
 * on screen sits in the range a proportion occupies. Two reasons for the caution:
 * a dimensionless quantity is not always a proportion — a retry multiplier and a
 * call depth are both `1` — and a value above one would then be labelled 340%,
 * which reads as a proportion that cannot exist rather than as the ratio it is.
 *
 * Nothing else is converted. Seconds are not turned into milliseconds and bytes
 * are not turned into mebibytes, because {@link formatSiNumber} already gives
 * those a magnitude prefix, and converting as well would apply the prefix twice.
 */
export function scaleFor(unit: string, values: number[]): Scale {
  const dimensionless = unit === '' || unit === '1'
  if (!dimensionless) return { factor: 1, suffix: unit }

  const finite = values.filter(Number.isFinite)
  if (finite.length === 0) return { factor: 1, suffix: '' }

  const lowest = Math.min(...finite)
  const highest = Math.max(...finite)
  const proportion = lowest >= 0 && highest <= 1.0000001
  return proportion ? { factor: 100, suffix: '%' } : { factor: 1, suffix: '' }
}

/** Renders one value on a chosen scale. */
export function showScaled(value: number, scale: Scale): string {
  if (!Number.isFinite(value)) return '—'
  const scaled = value * scale.factor
  // A percentage reads better with a fixed number of decimals than with the
  // significant figures a magnitude-prefixed number wants, because the range is
  // known and the prefix never applies.
  const text = scale.suffix === '%' ? trimZeros(scaled.toFixed(1)) : formatSiNumber(scaled)
  return scale.suffix === '%' ? `${text}%` : text
}

/** Renders a value with its unit written out, for places with room. */
export function showWithUnit(value: number, scale: Scale): string {
  const text = showScaled(value, scale)
  return scale.suffix && scale.suffix !== '%' ? `${text} ${scale.suffix}` : text
}

function trimZeros(text: string): string {
  return text.replace(/\.0$/, '')
}
