import { onBeforeUnmount, ref, type Ref } from 'vue'

/** Clearance from the anchor and from the edges of the window. */
const GAP = 12

/** A panel that hangs beside something, outside whatever would otherwise clip it. */
export interface Flyout {
  /** Where the panel sits, in viewport coordinates, or null while it is closed. */
  at: Ref<{ left: number; top: number } | null>
  open: () => void
  close: () => void
}

/**
 * Places a panel beside an element, in viewport coordinates.
 *
 * Viewport coordinates because the panels these fields live in scroll, and a
 * scrolling box crops whatever is positioned inside it — which is every pixel of
 * a flyout whose whole purpose is to hang outside the panel, where there is room
 * for it. Rendering at the document root and placing against the window is the
 * only frame of reference no ancestor can crop.
 *
 * `assumedWidth` is what the panel will measure once it exists. Without a guess
 * the first frame lands against the wrong edge and then jumps.
 */
export function useFlyout(
  anchor: () => HTMLElement | null,
  panel: () => HTMLElement | null,
  assumedWidth: number,
): Flyout {
  const at = ref<{ left: number; top: number } | null>(null)

  /**
   * A frame loop rather than scroll and resize listeners.
   *
   * The anchor moves for more reasons than a listener can enumerate — the panel
   * around it scrolls, the window resizes, the flyout itself grows from a line
   * of text into a chart when the expression resolves, rows above it appear and
   * disappear as the design changes underneath — and a `scroll` event does not
   * bubble, so every scrolling ancestor would need its own listener found,
   * attached and released again.
   *
   * Two rectangles a frame, for as long as one field has focus, is cheaper than
   * all of that and cannot be wrong. Nothing is written unless the placement
   * actually changed, so a still page costs no renders.
   */
  let frame = 0

  function settle() {
    const beside = anchor()
    if (!beside) return
    const rect = beside.getBoundingClientRect()
    const measured = panel()?.getBoundingClientRect()
    const width = measured?.width ?? assumedWidth
    const height = measured?.height ?? 0
    // Left where there is room for it, and the other side where there is not:
    // an anchor near the left edge would otherwise be covered by its own panel.
    const spare = rect.left - width - GAP >= GAP
    const beyond = window.innerWidth - width - GAP
    const placed = {
      left: Math.max(GAP, spare ? rect.left - width - GAP : Math.min(rect.right + GAP, beyond)),
      top: Math.max(GAP, Math.min(rect.top, window.innerHeight - height - GAP)),
    }
    if (at.value?.left === placed.left && at.value.top === placed.top) return
    at.value = placed
  }

  function track() {
    frame = requestAnimationFrame(track)
    settle()
  }

  function open() {
    settle()
    cancelAnimationFrame(frame)
    frame = requestAnimationFrame(track)
  }

  function close() {
    cancelAnimationFrame(frame)
    frame = 0
    at.value = null
  }

  onBeforeUnmount(close)

  return { at, open, close }
}
