import type { Component } from 'vue'
import {
  Box,
  Coin,
  Cpu,
  Grid,
  Lightning,
  Share,
  Tickets,
  User,
} from '@element-plus/icons-vue'

/**
 * What each kind of component looks like.
 *
 * The glyph is declared by the component type's manifest rather than guessed
 * from its identifier, so a project-local type gets the same treatment as a
 * shipped one. The names come from a closed vocabulary the server validates, and
 * anything outside it falls back to a plain box rather than to nothing: an
 * unrecognised type is still a component, and a diagram with a hole in it is
 * worse than one with an unremarkable square.
 */
const GLYPHS: Record<string, Component> = {
  client: User,
  service: Cpu,
  store: Coin,
  queue: Tickets,
  balancer: Share,
  aggregator: Grid,
  cache: Lightning,
  component: Box,
}

export function glyphFor(icon: string | undefined): Component {
  return GLYPHS[icon ?? 'component'] ?? Box
}

/**
 * The same vocabulary drawn as an outline, for a diagram that cannot host Vue.
 *
 * Cytoscape renders to a canvas, so a node cannot contain a component. These are
 * single-path SVG documents small enough to inline as a data URI, which is what
 * a node background accepts.
 */
const PATHS: Record<string, string> = {
  client: 'M12 12a5 5 0 100-10 5 5 0 000 10zm0 2c-5 0-9 2.5-9 5.5V22h18v-2.5c0-3-4-5.5-9-5.5z',
  service:
    'M9 2v2H7a3 3 0 00-3 3v2H2v2h2v2H2v2h2v2a3 3 0 003 3h2v2h2v-2h2v2h2v-2h2a3 3 0 003-3v-2h2v-2h-2v-2h2V9h-2V7a3 3 0 00-3-3h-2V2h-2v2h-2V2H9zm0 7h6v6H9V9z',
  store:
    'M12 2c-4.4 0-8 1.6-8 3.5S7.6 9 12 9s8-1.6 8-3.5S16.4 2 12 2zM4 8.6v3.9C4 14.4 7.6 16 12 16s8-1.6 8-3.5V8.6C18.3 9.9 15.4 10.6 12 10.6S5.7 9.9 4 8.6zm0 6v3.9C4 20.4 7.6 22 12 22s8-1.6 8-3.5v-3.9c-1.7 1.3-4.6 2-8 2s-6.3-.7-8-2z',
  queue: 'M3 5h18v3H3V5zm0 5.5h18v3H3v-3zM3 16h18v3H3v-3z',
  balancer:
    'M11 2h2v6h-2V2zm-8 12h2v8H3v-8zm16 0h2v8h-2v-8zm-8 0h2v8h-2v-8zM4 10h16v2h-2v2h-2v-2H8v2H6v-2H4v-2z',
  aggregator:
    'M3 3h8v8H3V3zm10 0h8v8h-8V3zM3 13h8v8H3v-8zm10 0h8v8h-8v-8z',
  cache: 'M13 2L4.5 13.5H11l-1 8.5 8.5-11.5H12l1-8.5z',
  component: 'M4 4h16v16H4V4zm2 2v12h12V6H6z',
}

/** The node the diagram draws, which the glyph image is composed to match. */
const NODE = { width: 148, height: 46 }

/** How big the glyph is drawn, and how far in from the node's left edge. */
const GLYPH = 17
const INSET = 13

/**
 * The glyph as a data URI sized to a diagram node.
 *
 * The image carries its own placement — a transparent canvas the shape of the
 * node, with the glyph drawn small and to the left — rather than being sized and
 * positioned by the diagram. Cytoscape's sizing properties apply only when an
 * image is drawn at its natural size, which its canvas renderer does not do
 * dependably for SVG; scaling an already-composed image to the node is one
 * instruction and behaves the same everywhere.
 */
export function glyphUri(icon: string | undefined, colour: string): string {
  const path = PATHS[icon ?? 'component'] ?? PATHS.component
  const scale = GLYPH / 24
  const top = (NODE.height - GLYPH) / 2
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="${NODE.width}" height="${NODE.height}" ` +
    `viewBox="0 0 ${NODE.width} ${NODE.height}">` +
    `<g transform="translate(${INSET} ${top}) scale(${scale})" fill="${colour}">` +
    `<path d="${path}"/></g></svg>`
  // Base64 rather than percent-encoded: both are valid data URIs, and canvas
  // renderers have historically been fussier about the latter. The glyphs are
  // ASCII, so the encoding is lossless.
  return `data:image/svg+xml;base64,${btoa(svg)}`
}
