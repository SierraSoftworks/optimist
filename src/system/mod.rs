//! Declarative definitions of the component kinds a system model is built from.
//!
//! # Why component kinds are data
//!
//! A capacity model is assembled from a small vocabulary of recurring parts: a
//! thing that generates demand, a thing that spreads it, a thing that buffers
//! it, a thing that serves it, a thing that stores it. Each has properties an
//! engineer can measure, derived quantities that follow from those properties,
//! and resource limits that decide when it becomes the bottleneck.
//!
//! Those definitions are data rather than code. The evaluator reads a component
//! type to discover what to compute; it never learns what a queue or a datastore
//! *is*. Adding a new kind of component is therefore a matter of writing a
//! manifest, not of changing the engine, and a project may introduce kinds the
//! catalogue never anticipated.
//!
//! # Anatomy of a type
//!
//! A type declares four things:
//!
//! - **Properties** are the intrinsic facts an author supplies, each carrying a
//!   unit annotation and, usually, an uncertain value. A service time, a
//!   connection limit, a retention window.
//! - **Channels** are quantities derived from properties, from the flows
//!   arriving on inbound relationships, and from the component's own state at
//!   the previous step. Each is a Squiggle expression evaluated over sample sets,
//!   so uncertainty flows through untouched.
//! - **Outputs** name the channels published onto outbound relationships, which
//!   is how one component's result becomes another's input.
//! - **Constraints** pair a demand channel with the limit it consumes, and are
//!   the whole point of the exercise. Every bottleneck the engine reports is a
//!   constraint whose demand has approached its limit.
//!
//! The engine attaches no meaning to any particular name. `throughput` is
//! whatever a manifest says it is, and a constraint called `iops` is ranked by
//! exactly the same arithmetic as one called `bandwidth`.

mod bottleneck;
mod catalogue;
mod comparison;
mod compile;
mod evaluate;
mod expression;
mod intervention;
mod manifest;
mod model;
mod mutator;
mod preview;
mod scale_unit;
mod schema;
mod signal;
mod validate;
mod values;

pub use bottleneck::{Bottleneck, bottlenecks, bottlenecks_with_mutators};
pub use catalogue::{CatalogueError, builtin_catalogue, builtin_mutators};
pub use comparison::{
    Comparison, Movement, compare, compare_many_with_mutators, compare_with_mutators,
};
pub use evaluate::{
    ComponentState, Evaluation, EvaluationConfig, EvaluationError, LinkId, LinkState, SolveMode,
    Step, evaluate, evaluate_intervention, evaluate_intervention_with_mutators,
    evaluate_with_mutators,
};
pub use intervention::{Intervention, InterventionId, Override};
pub use manifest::{
    Channel, ComponentType, ComponentTypeId, Constraint, Icon, Port, PortArity, Property,
};
pub use model::{Component, ComponentId, Position, Relationship, ScratchpadEntry, SystemModel};
pub use mutator::{AttachedMutator, Mutator, MutatorId, Transform};
pub use preview::preview;
pub use scale_unit::{Distribution, ScaleUnit, ScaleUnitId};
pub use schema::{
    ComponentDocument, LoadedSystem, OutgoingRelationship, SCHEMA_VERSION, SchemaError,
    SystemDocument, read_system, safe_identifier, write_system,
};
pub use signal::{Aggregation, Signal};
pub use validate::ComponentTypeError;

/// The quantities that may travel along a relationship, by name.
///
/// A port publishes signals rather than channels, so anything reporting what
/// arrived at a component or came back to it has no component type to read a
/// unit from. This is the vocabulary those names are drawn from.
pub fn signals() -> std::collections::BTreeMap<String, Signal> {
    signal::builtin_signals()
}
