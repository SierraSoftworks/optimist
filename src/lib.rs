//! Tools for collaboratively modelling causal systems and prioritizing interventions.
//!
//! Optimist separates its causal graph ([`domain`]) from project isolation
//! ([`project`]), persistence ([`store`]), transport ([`server`]), and command-line
//! interaction ([`cli`]). Keeping these boundaries explicit lets API clients, the web
//! interface, and analysis code share the same validated model.

#![deny(missing_docs)]

/// Command-line argument types and dispatch used by the `optimist` binary.
pub mod cli;
/// Revision-checked, idempotent graph mutation requests and outcomes.
pub mod command;
/// Strongly typed causal graph aggregates and embedded probabilistic values.
pub mod domain;
/// Versioned Markdown document schemas, bounded parsing, and canonical rendering.
pub mod markdown;
/// Project metadata and isolated graph lifecycle management.
pub mod project;
/// HTTP routing and server process lifecycle.
pub mod server;
/// Squiggle-compatible probabilistic language parsing and evaluation.
pub mod squiggle;
/// Backend-independent graph persistence contracts and implementations.
pub mod store;
