//! Tools for designing large systems and finding what constrains them.
//!
//! A design is a graph of typed components — clients, load balancers, queues,
//! compute pools, datastores — wired together and annotated with the properties
//! an engineer can measure. Optimist solves that graph with uncertainty carried
//! through it and reports which resource limits the design is closest to
//! exhausting.
//!
//! The pieces are separable. [`squiggle`] is a probabilistic language that knows
//! nothing about systems; [`system`] describes and solves designs; [`session`]
//! holds one in memory for several people to edit; [`api`] and [`cli`] are the
//! two ways to reach it.

#![deny(missing_docs)]

#[cfg(feature = "profiling")]
pub mod profile;
#[cfg(not(feature = "profiling"))]
mod profile;

/// HTTP and WebSocket access to a workspace of designs.
pub mod api;
/// Command-line argument types and dispatch used by the `optimist` binary.
pub mod cli;
/// Designs held in memory and shared by everyone editing them.
pub mod session;
/// Squiggle-compatible probabilistic language parsing and evaluation.
pub mod squiggle;
/// Declarative component types for non-abstract large system design.
pub mod system;
