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
mod compile;
mod evaluate;
mod expression;
mod manifest;
mod model;
mod mutator;
mod validate;
mod values;

pub use bottleneck::{Bottleneck, bottlenecks};
pub use catalogue::{CatalogueError, builtin_catalogue, builtin_mutators};
pub use evaluate::{ComponentState, Evaluation, EvaluationConfig, EvaluationError, Step, evaluate};
pub use manifest::{
    Channel, ComponentType, ComponentTypeId, Constraint, Port, PortArity, Property,
};
pub use model::{Component, ComponentId, Relationship, ScratchpadEntry, SystemModel};
pub use mutator::{AttachedMutator, Mutator, MutatorId, Transform};
pub use validate::ComponentTypeError;
