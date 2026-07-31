//! The individual checks a manifest's names and expressions must pass.

use std::collections::BTreeSet;

use crate::{
    squiggle::parse,
    system::{
        expression::free_names,
        manifest::Port,
        signal::{Publication, builtin_signals},
    },
};

use super::ComponentTypeError;

pub(super) fn validate_shaped_identifier(value: &str) -> Result<(), ComponentTypeError> {
    let shaped = !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
        && !value.starts_with('-')
        && !value.ends_with('-');
    shaped
        .then_some(())
        .ok_or_else(|| ComponentTypeError::Identifier {
            value: value.to_owned(),
        })
}

pub(super) fn validate_name(location: &str, name: &str) -> Result<(), ComponentTypeError> {
    let mut characters = name.chars();
    let shaped = characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_');
    shaped
        .then_some(())
        .ok_or_else(|| ComponentTypeError::Name {
            location: location.to_owned(),
            value: name.to_owned(),
        })
}

pub(super) fn validate_unit(location: &str, unit: &str) -> Result<(), ComponentTypeError> {
    // A unit is checked by placing it where the language expects one.
    parse(&format!("value :: {unit} = 1\nvalue"))
        .map(|_| ())
        .map_err(|_| ComponentTypeError::Unit {
            location: location.to_owned(),
            value: unit.to_owned(),
        })
}

pub(super) fn validate_syntax(location: &str, source: &str) -> Result<(), ComponentTypeError> {
    parse(source)
        .map(|_| ())
        .map_err(|diagnostics| ComponentTypeError::Syntax {
            location: location.to_owned(),
            message: first_message(&diagnostics),
        })
}

pub(super) fn validate_references(
    location: &str,
    source: &str,
    visible: &BTreeSet<String>,
) -> Result<(), ComponentTypeError> {
    let free = free_names(source).map_err(|diagnostics| ComponentTypeError::Syntax {
        location: location.to_owned(),
        message: first_message(&diagnostics),
    })?;
    for name in free {
        if !visible.contains(&name) {
            return Err(ComponentTypeError::Unresolved {
                location: location.to_owned(),
                value: name,
            });
        }
    }
    Ok(())
}

/// Checks a port's published signals against the sides each may travel from.
///
/// Two component types are interchangeable only because they publish the same
/// signals from the same side, and nothing about a design says so: a missing
/// latency reads as an instant answer and a missing success as one that cannot
/// fail. Both are silent, and both flatter the design.
pub(super) fn validate_publication(
    location: &str,
    port: &Port,
    inbound: bool,
) -> Result<(), ComponentTypeError> {
    for (name, signal) in builtin_signals() {
        let (rule, other) = if inbound {
            (signal.publish.inbound, signal.publish.outbound)
        } else {
            (signal.publish.outbound, signal.publish.inbound)
        };
        match (rule, port.publishes.contains_key(name)) {
            (Publication::Forbidden, true) => {
                return Err(ComponentTypeError::Undeliverable {
                    location: location.to_owned(),
                    signal: name.clone(),
                    instead: (other != Publication::Forbidden).then(|| {
                        if inbound {
                            "an outbound port".to_owned()
                        } else {
                            "an inbound port".to_owned()
                        }
                    }),
                });
            }
            (Publication::Required, false) => {
                return Err(ComponentTypeError::Unpublished {
                    location: location.to_owned(),
                    signal: name.clone(),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn first_message(diagnostics: &[crate::squiggle::Diagnostic]) -> String {
    diagnostics.first().map_or_else(
        || "invalid expression".to_owned(),
        |first| first.message.clone(),
    )
}
