//! A self-contained Squiggle-compatible probabilistic language runtime.
//!
//! This module owns its syntax tree, parser, values, evaluator, and statistical
//! operations. It does not depend on Optimist's persisted domain model, so callers
//! can use it as a sidecar interpreter or adapt its results explicitly.

pub mod ast;
pub mod diagnostic;
pub mod distribution;
pub mod runtime;
pub mod value;

mod lexer;
mod lint;
mod parse;
mod parser;
mod token;

pub use diagnostic::{Diagnostic, DiagnosticKind};
pub use distribution::Distribution;
pub use lint::{lint, lint_program};
pub use parse::parse;
pub use runtime::{ModuleOutput, Runtime, RuntimeConfig, builtin_names};
pub use value::{DateValue, Domain, DurationValue, Function, Value};
