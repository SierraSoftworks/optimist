//! Load-time checking of component type definitions.
//!
//! Validation runs once when a catalogue loads. Everything it proves is
//! something the evaluator would otherwise have to re-check on every draw of
//! every step, or worse, discover as a failure partway through a run.

mod checks;
mod component;
mod mutator;

use std::fmt;

/// Why a component type definition cannot be used.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComponentTypeError {
    /// The identifier is empty or uses characters outside the accepted set.
    Identifier {
        /// The rejected identifier.
        value: String,
    },
    /// A declared name is not a usable Squiggle binding.
    Name {
        /// Where the name was declared.
        location: String,
        /// The rejected name.
        value: String,
    },
    /// One name is declared as both a property and a channel.
    Duplicate {
        /// The name declared twice.
        value: String,
    },
    /// An expression could not be parsed.
    Syntax {
        /// Where the expression was declared.
        location: String,
        /// The first parser diagnostic.
        message: String,
    },
    /// An expression refers to a name the evaluator will not supply.
    Unresolved {
        /// Where the expression was declared.
        location: String,
        /// The unresolved name.
        value: String,
    },
    /// A unit annotation could not be parsed.
    Unit {
        /// Where the annotation was declared.
        location: String,
        /// The rejected annotation.
        value: String,
    },
    /// An output names a quantity that does not exist.
    Output {
        /// The signal being published.
        signal: String,
        /// The quantity it names.
        channel: String,
    },
}

impl fmt::Display for ComponentTypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identifier { value } => write!(
                formatter,
                "'{value}' is not a valid component type identifier; use lower-case words joined by hyphens"
            ),
            Self::Name { location, value } => write!(
                formatter,
                "{location} '{value}' is not a valid name; use a letter followed by letters, digits, or underscores"
            ),
            Self::Duplicate { value } => write!(
                formatter,
                "'{value}' is declared as both a property and a channel"
            ),
            Self::Syntax { location, message } => {
                write!(formatter, "{location} does not parse: {message}")
            }
            Self::Unresolved { location, value } => write!(
                formatter,
                "{location} refers to '{value}', which is not a property, a channel, or a reserved binding"
            ),
            Self::Unit { location, value } => {
                write!(formatter, "{location} has an invalid unit '{value}'")
            }
            Self::Output { signal, channel } => write!(
                formatter,
                "output '{signal}' publishes '{channel}', which is not a property or a channel"
            ),
        }
    }
}

impl std::error::Error for ComponentTypeError {}
