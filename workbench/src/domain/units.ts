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
 * The only rescaling done is share-to-percent. A share is a proportion of a
 * whole and says so in its declaration, which is what separates a success of
 * `0.97` from a fan-out of `3`: both are pure numbers, and only one of them is
 * ninety-seven percent of anything.
 *
 * Nothing else is converted. Seconds are not turned into milliseconds and bytes
 * are not turned into mebibytes, because {@link formatSiNumber} already gives
 * those a magnitude prefix, and converting as well would apply the prefix twice.
 */
export function scaleFor(unit: string): Scale {
  if (PROPORTIONS.has(unit)) return { factor: 100, suffix: '%' }
  // The dimensionless annotation names no unit, so there is nothing to write.
  return { factor: 1, suffix: unit === '1' ? '' : unit }
}

/** Spellings the manifests accept for a proportion of a whole. */
const PROPORTIONS = new Set(['share', 'ratio', 'fraction', 'proportion', 'probability', '%'])

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
