//! Checking a behaviour's declarations against each other.

use std::collections::BTreeSet;

use crate::system::{expression::MUTATOR_RESERVED, mutator::Mutator};

use super::{
    ComponentTypeError,
    checks::{
        validate_name, validate_references, validate_shaped_identifier, validate_syntax,
        validate_unit,
    },
};

impl Mutator {
    /// Parses and validates a mutator from its YAML manifest.
    ///
    /// ```
    /// use optimist::system::Mutator;
    ///
    /// let manifest = "
    /// id: sample
    /// name: Sampling
    /// properties:
    ///   ratio:
    ///     unit: '1'
    /// requests:
    ///   rate:
    ///     unit: op/s
    ///     expression: signal.rate * ratio
    /// responses:
    ///   latency:
    ///     unit: s
    ///     expression: signal.latency * ratio
    /// ";
    /// let mutator = Mutator::parse(manifest)?;
    ///
    /// assert_eq!(mutator.id.as_str(), "sample");
    /// // `requests` rewrites the flow on its way downstream, `responses` on the
    /// // way back, so one definition can both raise demand and answer for it.
    /// assert!(mutator.requests.contains_key("rate"));
    /// assert!(mutator.responses.contains_key("latency"));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn parse(manifest: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mutator: Self = serde_yaml_ng::from_str(manifest)?;
        mutator.validate()?;
        Ok(mutator)
    }

    /// Checks every invariant the evaluator relies on.
    pub fn validate(&self) -> Result<(), ComponentTypeError> {
        validate_shaped_identifier(self.id.as_str())?;
        let mut visible = MUTATOR_RESERVED
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<BTreeSet<_>>();
        for (name, property) in &self.properties {
            validate_name("property", name)?;
            validate_unit(&format!("property '{name}'"), &property.unit)?;
            if let Some(default) = &property.default {
                validate_syntax(&format!("property '{name}' default"), default)?;
            }
            visible.insert(name.clone());
        }
        for (name, transform) in &self.requests {
            validate_name("request", name)?;
            validate_unit(&format!("request '{name}'"), &transform.unit)?;
            validate_references(
                &format!("request '{name}'"),
                &transform.expression,
                &visible,
            )?;
        }
        for (name, transform) in &self.responses {
            validate_name("response", name)?;
            validate_unit(&format!("response '{name}'"), &transform.unit)?;
            validate_references(
                &format!("response '{name}'"),
                &transform.expression,
                &visible,
            )?;
        }
        Ok(())
    }
}
