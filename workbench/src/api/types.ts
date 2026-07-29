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
  /** Outbound port on `from`; absent when the type declares exactly one. */
  from_port?: string
  /** Inbound port on `to`; absent when the type declares exactly one. */
  to_port?: string
  /**
   * Squiggle source for how many operations may wait on the wire.
   *
   * Absent means the server's default, so a link nobody has tuned stays
   * untouched in the design's YAML rather than gaining an expression that
   * merely restates that default.
   */
  capacity?: string
  /**
   * Squiggle source for how fast the wire carries bytes.
   *
   * Absent leaves the link unlimited, so it reports no constraint at all rather
   * than one nobody stated.
   */
  bandwidth?: string
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

/**
 * How demand meets the replicas of a scale unit.
 *
 * Sharded traffic divides between them; mirrored traffic reaches all of them, so
 * the replica count multiplies cost without dividing load.
 */
export type Distribution = 'sharded' | 'mirrored'

/** A group of components replicated together. */
export interface ScaleUnit {
  id: string
  name: string
  summary: string
  /** An expression, so a fleet size can be a shared quantity or a range. */
  replicas: string
  members: string[]
  distribution: Distribution
  /**
   * The unit this one sits inside, where it is itself replicated. Absent at the
   * outermost level, which is where a chain of enclosing units ends.
   */
  parent?: string | null
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

/**
 * Whether a quantity leads a component's numbers or sits behind them.
 *
 * `key` is a service level somebody depending on the component experiences;
 * `supporting` is an operational figure it was derived from.
 */
export type Emphasis = 'key' | 'supporting'

/** A quantity a component type computes. */
export interface ChannelDefinition {
  unit: string
  summary: string
  emphasis: Emphasis
  expression: string
}

/** A limit a component type can reach. */
export interface ConstraintDefinition {
  summary: string
  emphasis: Emphasis
  demand: string
  limit: string
}

/** How many relationships one side of a component type accepts. */
/** A named place relationships attach to a component. */
export interface Port {
  arity: string
  summary: string
  /** Signals this port puts on the wire, keyed by signal name. */
  publishes: Record<string, string>
}

/** The named places relationships attach, by side. */
export interface Ports {
  /** Ports callers attach to, receiving requests and publishing responses. */
  in: Record<string, Port>
  /** Ports dependencies attach to, publishing requests and receiving responses. */
  out: Record<string, Port>
}

/** One quantity that may travel along a relationship. */
export interface SignalDefinition {
  unit: string
  summary: string
  aggregate: string
  extensive: boolean
}

/** A kind of component a design may use. */
export interface ComponentType {
  id: string
  name: string
  summary: string
  /**
   * Which glyph stands for this kind of component.
   *
   * From a closed vocabulary the server validates, so anything unrecognised here
   * means a workbench older than the design it is reading.
   */
  icon: string
  ports: Ports
  properties: Record<string, PropertyDefinition>
  channels: Record<string, ChannelDefinition>
  constraints: Record<string, ConstraintDefinition>
}

/** A behaviour a relationship may carry. */
export interface MutatorType {
  id: string
  name: string
  summary: string
  properties: Record<string, PropertyDefinition>
  /** Signals rewritten on the way to the dependency. */
  requests: Record<string, { unit: string; summary: string; expression: string }>
  /** Signals rewritten on the way back to the caller. */
  responses: Record<string, { unit: string; summary: string; expression: string }>
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
  /**
   * The quantities that travel along a relationship, by name.
   *
   * A port publishes signals rather than channels, so anything showing what
   * arrived or what came back has no component type to read a unit from.
   */
  signals: Record<string, SignalDefinition>
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
  /**
   * The relationship owning the constraint, where one does.
   *
   * A wire's limits belong to neither end of it, so `component` names the
   * component it leaves and this says which relationship.
   */
  link?: string
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

/**
 * A step that did not settle, and what was still moving when the solver stopped.
 *
 * Reported alongside `Analysis.converged` because that flag is a claim about
 * every step of a horizon while `iterations` belongs to the last one, and a
 * surge that has passed leaves the last step settling in a pass or two.
 */
export interface Moving {
  time: number
  iterations: number
  component: string
  channel: string
  movement: number
  stalled: boolean
}

/** A step whose draws settled on several states rather than one. */
export interface Mixed {
  time: number
  component: string
  channel: string
  states: number
}

/** A solved design and what constrains it. */
export interface Analysis {
  sequence: number
  converged: boolean
  iterations: number
  /** Present only where some step failed to settle. */
  moving?: Moving
  /** Present only where the design settled on several states. */
  mixed?: Mixed
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

/** Which question a running solve is answering. */
export type SolveKind = 'analysis' | 'comparison'

/** Which solve a message is about. */
export interface SolveTarget {
  kind: SolveKind
  /** The variant being solved, or null for the design as it stands. */
  variant: string | null
  /** Where the design stood when the solve started. */
  sequence: number
}

/**
 * A solve running on the server right now.
 *
 * Solves are reported to everyone watching a design rather than to whoever asked
 * for one, because the server answers each question once: two people looking at
 * the same variant are waiting on the same arithmetic, and somebody who reloads
 * the page has not stopped waiting for it.
 */
export interface RunningSolve extends SolveTarget {
  /** How much of it appears to be done, in 0..=1. */
  fraction: number
  /** The timestep being relaxed, counted from one. */
  step: number
  /** How many timesteps the horizon holds. */
  steps: number
  /** Passes taken over that timestep. */
  pass: number
  /** The quantity the relaxation is still waiting on. */
  moving?: { component: string; channel: string }
}

/**
 * A message on a design's change feed.
 *
 * `snapshot` arrives first and describes the design as it stood when the socket
 * opened, followed by `active` listing the solves already under way. `change`
 * carries somebody's edit, which a client applies to its own copy rather than
 * refetching, so that an edit in progress elsewhere on the page is not
 * discarded. `solving` and `solved` say what the server is working on. `lagged`
 * means the client fell far enough behind that changes were dropped and only a
 * refetch can make it whole.
 *
 * The discriminator is `type` here and `kind` on a mutation. They are different
 * enums on the server and the names follow it rather than being unified, so that
 * a message can be passed to a mutation handler without being rewritten.
 */
export type FeedMessage =
  | ({ type: 'snapshot' } & Snapshot)
  | { type: 'active'; solves: RunningSolve[] }
  | { type: 'change'; sequence: number; mutation: Mutation }
  | { type: 'solving'; solve: RunningSolve }
  | { type: 'solved'; solve: SolveTarget }
  | { type: 'lagged'; missed: number }
