//! Load-time checking of component type definitions.
//!
//! Validation runs once when a catalogue loads. Everything it proves is
//! something the evaluator would otherwise have to re-check on every draw of
//! every step, or worse, discover as a failure partway through a run.

use std::{collections::BTreeSet, fmt};

use crate::squiggle::parse;

use super::{
    expression::{RESERVED, free_names},
    manifest::{ComponentType, ComponentTypeId},
};

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

impl ComponentType {
    /// Parses and validates a component type from its YAML manifest.
    ///
    /// ```
    /// use optimist::system::ComponentType;
    ///
    /// let manifest = "
    /// id: token-bucket
    /// name: Token bucket
    /// properties:
    ///   refill:
    ///     unit: op/s
    ///   burst:
    ///     unit: op
    /// channels:
    ///   admitted:
    ///     unit: op/s
    ///     expression: min([inbound.rate, refill])
    /// outputs:
    ///   rate: admitted
    /// constraints:
    ///   throughput:
    ///     demand: inbound.rate
    ///     limit: refill
    /// ";
    /// let component = ComponentType::parse(manifest)?;
    /// assert_eq!(component.id.as_str(), "token-bucket");
    /// assert!(component.properties["refill"].is_required());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn parse(manifest: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let component: Self = serde_yaml_ng::from_str(manifest)?;
        component.validate()?;
        Ok(component)
    }

    /// Checks every invariant the evaluator relies on.
    pub fn validate(&self) -> Result<(), ComponentTypeError> {
        validate_identifier(&self.id)?;
        let mut surface = BTreeSet::new();
        for (name, property) in &self.properties {
            validate_name("property", name)?;
            validate_unit(&format!("property '{name}'"), &property.unit)?;
            if let Some(default) = &property.default {
                validate_syntax(&format!("property '{name}' default"), default)?;
            }
            surface.insert(name.clone());
        }
        for (name, channel) in &self.channels {
            validate_name("channel", name)?;
            validate_unit(&format!("channel '{name}'"), &channel.unit)?;
            if !surface.insert(name.clone()) {
                return Err(ComponentTypeError::Duplicate {
                    value: name.clone(),
                });
            }
        }

        let visible = surface
            .iter()
            .cloned()
            .chain(RESERVED.iter().map(|name| (*name).to_owned()))
            .collect::<BTreeSet<_>>();
        for (name, channel) in &self.channels {
            validate_references(&format!("channel '{name}'"), &channel.expression, &visible)?;
        }
        for (name, constraint) in &self.constraints {
            validate_references(
                &format!("constraint '{name}' demand"),
                &constraint.demand,
                &visible,
            )?;
            validate_references(
                &format!("constraint '{name}' limit"),
                &constraint.limit,
                &visible,
            )?;
        }
        for (signal, channel) in &self.outputs {
            validate_name("output", signal)?;
            // A published quantity may be derived or intrinsic: a payload size is
            // a property of the component, not something it computes.
            if !surface.contains(channel) {
                return Err(ComponentTypeError::Output {
                    signal: signal.clone(),
                    channel: channel.clone(),
                });
            }
        }
        Ok(())
    }
}

fn validate_identifier(id: &ComponentTypeId) -> Result<(), ComponentTypeError> {
    let value = id.as_str();
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

fn validate_name(location: &str, name: &str) -> Result<(), ComponentTypeError> {
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

fn validate_unit(location: &str, unit: &str) -> Result<(), ComponentTypeError> {
    // A unit is checked by placing it where the language expects one.
    parse(&format!("value :: {unit} = 1\nvalue"))
        .map(|_| ())
        .map_err(|_| ComponentTypeError::Unit {
            location: location.to_owned(),
            value: unit.to_owned(),
        })
}

fn validate_syntax(location: &str, source: &str) -> Result<(), ComponentTypeError> {
    parse(source)
        .map(|_| ())
        .map_err(|diagnostics| ComponentTypeError::Syntax {
            location: location.to_owned(),
            message: diagnostics.first().map_or_else(
                || "invalid expression".to_owned(),
                |first| first.message.clone(),
            ),
        })
}

fn validate_references(
    location: &str,
    source: &str,
    visible: &BTreeSet<String>,
) -> Result<(), ComponentTypeError> {
    let free = free_names(source).map_err(|diagnostics| ComponentTypeError::Syntax {
        location: location.to_owned(),
        message: diagnostics.first().map_or_else(
            || "invalid expression".to_owned(),
            |first| first.message.clone(),
        ),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(body: &str) -> String {
        format!("id: probe\nname: Probe\n{body}")
    }

    #[test]
    fn a_minimal_type_validates() {
        let component = ComponentType::parse(&manifest(
            "properties:\n  limit:\n    unit: op/s\nchannels:\n  served:\n    unit: op/s\n    expression: min([inbound.rate, limit])\n",
        ))
        .expect("valid");
        assert_eq!(component.channels.len(), 1);
    }

    #[test]
    fn a_property_without_a_default_is_required() {
        let component = ComponentType::parse(&manifest(
            "properties:\n  limit:\n    unit: op/s\n  spare:\n    unit: op/s\n    default: '0'\n",
        ))
        .expect("valid");
        assert!(component.properties["limit"].is_required());
        assert!(!component.properties["spare"].is_required());
    }

    #[test]
    fn a_mistyped_reference_is_caught_at_load_time() {
        let error = ComponentType::parse(&manifest(
            "properties:\n  limit:\n    unit: op/s\nchannels:\n  served:\n    unit: op/s\n    expression: min([inbound.rate, limitt])\n",
        ))
        .expect_err("unresolved");
        assert!(error.to_string().contains("limitt"), "{error}");
    }

    #[test]
    fn reserved_bindings_resolve() {
        ComponentType::parse(&manifest(
            "properties:\n  drain:\n    unit: op/s\nchannels:\n  backlog:\n    unit: op\n    expression: max([prev.backlog + (inbound.rate - drain) * dt, 0])\n  age:\n    unit: s\n    expression: t\n",
        ))
        .expect("valid");
    }

    #[test]
    fn a_channel_may_reference_another_channel() {
        ComponentType::parse(&manifest(
            "channels:\n  arrivals:\n    unit: op/s\n    expression: inbound.rate\n  doubled:\n    unit: op/s\n    expression: arrivals * 2\n",
        ))
        .expect("valid");
    }

    #[test]
    fn a_name_declared_twice_is_rejected() {
        let error = ComponentType::parse(&manifest(
            "properties:\n  rate:\n    unit: op/s\nchannels:\n  rate:\n    unit: op/s\n    expression: '1'\n",
        ))
        .expect_err("duplicate");
        assert!(
            error.to_string().contains("both a property and a channel"),
            "{error}"
        );
    }

    #[test]
    fn an_output_must_name_a_channel() {
        let error = ComponentType::parse(&manifest(
            "channels:\n  served:\n    unit: op/s\n    expression: inbound.rate\noutputs:\n  rate: missing\n",
        ))
        .expect_err("output");
        assert!(
            error.to_string().contains("not a property or a channel"),
            "{error}"
        );
    }

    #[test]
    fn an_output_may_publish_a_property() {
        ComponentType::parse(&manifest(
            "properties:\n  payload:\n    unit: B/op\noutputs:\n  payload: payload\n",
        ))
        .expect("valid");
    }

    #[test]
    fn a_broken_expression_is_rejected() {
        let error = ComponentType::parse(&manifest(
            "channels:\n  served:\n    unit: op/s\n    expression: 'inbound.rate *'\n",
        ))
        .expect_err("syntax");
        assert!(error.to_string().contains("does not parse"), "{error}");
    }

    #[test]
    fn identifiers_and_names_are_shaped() {
        assert!(ComponentType::parse("id: Not Valid\nname: X\n").is_err());
        assert!(
            ComponentType::parse("id: probe\nname: X\nproperties:\n  '2bad':\n    unit: op\n")
                .is_err()
        );
    }

    #[test]
    fn constraints_are_checked_like_channels() {
        let error = ComponentType::parse(&manifest(
            "properties:\n  limit:\n    unit: op/s\nconstraints:\n  throughput:\n    demand: offered\n    limit: limit\n",
        ))
        .expect_err("unresolved");
        assert!(error.to_string().contains("offered"), "{error}");
    }
}
