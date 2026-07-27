/**
 * Wire types for the design API.
 *
 * These mirror the Rust structures the server serialises. Where a name differs
 * from the Rust one it is because the JSON is snake_case and stays that way:
 * translating field names on the way in buys nothing and makes a mismatch
 * between client and server show up as `undefined` at render time rather than as
 * a type error at build time.
 */

/** A design as it appears in the workspace listing. */
export interface DesignSummary {
  id: string
  name: string
  summary: string
  /**
   * Why the design could not be read, where it could not.
   *
   * A design that fails to load is still listed. Hiding it would leave somebody
   * unable to find a design they know exists, with no way to discover that its
   * file is malformed.
   */
  unreadable?: string
}

/** A shared quantity every part of a design can refer to. */
export interface ScratchpadEntry {
  name: string
  expression: string
  unit: string
  summary: string
}

/** A behaviour attached to a relationship. */
export interface AttachedMutator {
  type: string
  properties: Record<string, string>
}

/** One direction of flow between two components. */
export interface Relationship {
  from: string
  to: string
  summary: string
  mutators: AttachedMutator[]
}

/** Where a component sits on the diagram, once somebody has placed it. */
export interface Position {
  x: number
  y: number
}

/** A thing in the design that carries capacity or demand. */
export interface Component {
  id: string
  name: string
  type: string
  summary?: string
  properties: Record<string, string>
  /**
   * Absent until the component is moved, so an unarranged design is laid out
   * automatically rather than pinned to whatever an algorithm produced first.
   */
  position?: Position
}

/** A group of components replicated together. */
export interface ScaleUnit {
  id: string
  name: string
  summary: string
  replicas: string
  members: string[]
  distribution: string
}

/** A proposal, expressed as replacements for shared quantities. */
export interface Intervention {
  id: string
  name: string
  summary: string
  overrides: { name: string; expression: string }[]
}

/** The whole editable design. */
export interface SystemModel {
  scratchpad: ScratchpadEntry[]
  components: Component[]
  relationships: Relationship[]
  scale_units: ScaleUnit[]
  interventions: Intervention[]
}

/** A design and where it sits in its change feed. */
export interface Snapshot {
  name: string
  summary: string
  model: SystemModel
  sequence: number
}

/** A property a component type expects to be given. */
export interface PropertyDefinition {
  unit: string
  summary: string
  default?: string | null
}

/** A quantity a component type computes. */
export interface ChannelDefinition {
  unit: string
  summary: string
  expression: string
}

/** A limit a component type can reach. */
export interface ConstraintDefinition {
  summary: string
  demand: string
  limit: string
}

/** How many relationships one side of a component type accepts. */
export interface Port {
  arity: string
  summary: string
}

/** A kind of component a design may use. */
export interface ComponentType {
  id: string
  name: string
  summary: string
  inbound?: Port | null
  outbound?: Port | null
  properties: Record<string, PropertyDefinition>
  channels: Record<string, ChannelDefinition>
  constraints: Record<string, ConstraintDefinition>
  outputs: Record<string, string>
}

/** A behaviour a relationship may carry. */
export interface MutatorType {
  id: string
  name: string
  summary: string
  properties: Record<string, PropertyDefinition>
  transforms: Record<string, { unit: string; summary: string; expression: string }>
}

/**
 * Everything a design may build from.
 *
 * Both catalogues are keyed by identifier rather than listed, because every use
 * of them is a lookup from a component's declared type.
 */
export interface Catalogue {
  component_types: Record<string, ComponentType>
  mutators: Record<string, MutatorType>
  /** Every name an expression may call, for the editor to complete against. */
  builtins: string[]
}

/**
 * A solved quantity.
 *
 * `draws` is empty when the quantity is certain. That is a statement about the
 * quantity rather than about the response: render it as a point, not as a
 * spread of zero width.
 */
export interface Quantity {
  mean: number
  p10: number
  p50: number
  p90: number
  draws: number[]
}

/** How heavily one constraint is loaded. */
export interface Bottleneck {
  component: string
  constraint: string
  summary: string
  replicas: number
  utilisation: number
  utilisation_p90: number
  probability_of_binding: number
  headroom: number
}

/** Every solved channel at one moment, by component and then by channel. */
export type Solved = Record<string, Record<string, Quantity>>

/** One moment in a design's history. */
export interface Frame {
  time: number
  converged: boolean
  components: Solved
}

/** A solved design and what constrains it. */
export interface Analysis {
  sequence: number
  converged: boolean
  iterations: number
  components: Solved
  /** Present only where the caller asked for a series. */
  series?: Frame[]
  bottlenecks: Bottleneck[]
}

/** What a proposal did to one constraint. */
export interface Movement {
  component: string
  constraint: string
  /** Mean utilisation without the proposal. */
  before: number
  /** Mean utilisation with it. */
  after: number
  /** Share of draws that bound without the proposal. */
  bound_before: number
  /** Share of draws that bind with it. */
  bound_after: number
}

/** A proposal weighed against the design it would replace. */
export interface Comparison {
  intervention: string
  movements: Movement[]
}

/**
 * An edit to a design.
 *
 * The shapes match the server's tagged enum exactly, because these are also the
 * messages other editors receive over the feed. A client that invented its own
 * edit format would have to translate twice and would still be unable to apply
 * someone else's.
 */
export type Mutation =
  | { kind: 'set_scratchpad_entry'; entry: ScratchpadEntry }
  | { kind: 'remove_scratchpad_entry'; name: string }
  | { kind: 'set_component'; component: Component }
  | { kind: 'remove_component'; id: string }
  | { kind: 'set_relationship'; relationship: Relationship }
  | { kind: 'remove_relationship'; from: string; to: string }
  | { kind: 'set_scale_unit'; scale_unit: ScaleUnit }
  | { kind: 'remove_scale_unit'; id: string }
  | { kind: 'set_intervention'; intervention: Intervention }
  | { kind: 'remove_intervention'; id: string }

/** What the server says after an edit lands. */
export interface Applied {
  sequence: number
  applied: number
}

/**
 * A message on a design's change feed.
 *
 * `snapshot` arrives first and describes the design as it stood when the socket
 * opened. `change` carries somebody's edit, which a client applies to its own
 * copy rather than refetching, so that an edit in progress elsewhere on the page
 * is not discarded. `lagged` means the client fell far enough behind that
 * changes were dropped and only a refetch can make it whole.
 *
 * The discriminator is `type` here and `kind` on a mutation. They are different
 * enums on the server and the names follow it rather than being unified, so that
 * a message can be passed to a mutation handler without being rewritten.
 */
export type FeedMessage =
  | ({ type: 'snapshot' } & Snapshot)
  | { type: 'change'; sequence: number; mutation: Mutation }
  | { type: 'lagged'; missed: number }
