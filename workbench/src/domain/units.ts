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
  // A percentage has a known range and never takes a magnitude prefix, so it is
  // written out in full at the precision {@link showPercentage} judges it worth.
  if (scale.suffix === '%') return `${showPercentage(scaled)}%`
  return formatSiNumber(scaled)
}

/**
 * Writes a percentage at the precision its distance from an end earns.
 *
 * A share is read through its distance from an end rather than through its own
 * digits: 99.99% and 99.9999% are two orders of magnitude apart in what they
 * cost, and a fixed decimal place writes both as `100%`. The inverse of that
 * distance, `100 / min(p, 100 - p)`, grows tenfold for every nine added to a
 * service level, so its base-ten logarithm counts them; one is subtracted
 * because the first nine is already written by the integer part.
 *
 * Taking the nearer of the two ends means a share that is almost nothing keeps
 * its magnitude for the same reason a share that is almost everything does: a
 * failure rate of 0.002% is not the same event as one of 0.2%, and rounding
 * both to `0%` throws away the only part anybody was reading.
 *
 * Precision stops at {@link RESOLUTION}, where a solved figure stops meaning
 * anything, so a share a hair away from an end is written as that end rather
 * than as a row of digits standing for sampling noise.
 */
function showPercentage(percent: number): string {
  const distance = Math.min(Math.abs(percent), Math.abs(100 - percent))
  const places =
    distance === 0 ? 0 : Math.min(Math.max(Math.floor(Math.log10(100 / distance)) - 1, 0), RESOLUTION)
  return trimZeros(percent.toFixed(places))
}

/** Decimal places past which a solved share is reporting sampling noise. */
const RESOLUTION = 6

/** Renders a value with its unit written out, for places with room. */
export function showWithUnit(value: number, scale: Scale): string {
  const text = showScaled(value, scale)
  return scale.suffix && scale.suffix !== '%' ? `${text} ${scale.suffix}` : text
}

function trimZeros(text: string): string {
  return text.includes('.') ? text.replace(/\.?0+$/, '') : text
}
