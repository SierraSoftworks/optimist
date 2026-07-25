use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

use super::{Estimate, QuantityDefinition, QuantityError, QuantityValue, RelationError};

/// Largest relation source accepted, matching the estimate source limit.
const MAX_SOURCE_BYTES: usize = 65_536;

/// Names the generated binding prelude owns and a parameter may not shadow.
const RESERVED_NAMES: [&str; 3] = ["baseline", "optimist_result", "optimist_bindings"];

/// One uncertain coefficient owned by a relation and referenced by name.
///
/// A parameter is where a relation's uncertainty belongs. The relation itself is
/// a deterministic function of already-sampled inputs, so authoring
/// `normal(1, 0.1)` inside the source is rejected; naming it here instead lets
/// propagation sample it once per draw and hold it constant across periods,
/// which is what makes a coefficient a coefficient rather than per-period noise.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RelationParameter {
    /// Unit, support, and operational definition the coefficient is checked against.
    pub quantity: QuantityDefinition,
    /// Uncertain value, sampled once per Monte Carlo draw.
    pub value: Estimate<QuantityValue>,
}

/// A node equation computing one state's value from its parents each period.
///
/// A relation replaces proportional composition for the state that owns it: the
/// authored arithmetic decides how parents combine, so incoming relationship
/// responses no longer scale anything. Those relationships still declare which
/// parents exist and how far they lag, and intervention effects still supply
/// their activation, but the magnitudes come from here.
///
/// The source is stored as authored and compiled against a schema derived from
/// the graph. It is not compiled during deserialization because the parents a
/// relation may reference are a property of the graph rather than of the node,
/// and no graph is available while a node is being decoded.
///
/// ```
/// use optimist::domain::StateRelation;
///
/// let relation = StateRelation::new(
///     "outage_frequency * impact_duration".to_owned(),
///     Default::default(),
/// )?;
/// assert_eq!(relation.source, "outage_frequency * impact_duration");
/// # Ok::<(), optimist::domain::StateRelationError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StateRelation {
    /// Authored Squiggle source, evaluated against a generated binding prelude.
    pub source: String,
    /// Uncertain coefficients this relation may reference, keyed by binding name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, RelationParameter>,
}

impl StateRelation {
    /// Creates a relation after checking its source bounds and parameter names.
    ///
    /// Whether the arithmetic type-checks depends on the graph, so that is
    /// verified when the relation is attached to a project rather than here.
    pub fn new(
        source: String,
        parameters: BTreeMap<String, RelationParameter>,
    ) -> Result<Self, StateRelationError> {
        let source = source.trim().to_owned();
        if source.is_empty() {
            return Err(StateRelationError::EmptySource);
        }
        if source.len() > MAX_SOURCE_BYTES {
            return Err(StateRelationError::SourceTooLarge);
        }
        for (name, parameter) in &parameters {
            if !is_binding_name(name) {
                return Err(StateRelationError::InvalidParameterName(name.clone()));
            }
            if RESERVED_NAMES.contains(&name.as_str()) {
                return Err(StateRelationError::ReservedParameterName(name.clone()));
            }
            let quantity = parameter.quantity.clone().validated()?;
            if !quantity.accepts(&parameter.value.distribution) {
                return Err(StateRelationError::ParameterOutsideSupport(name.clone()));
            }
        }
        Ok(Self { source, parameters })
    }
}

fn is_binding_name(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StateRelationWire {
    source: String,
    #[serde(default)]
    parameters: BTreeMap<String, RelationParameter>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationParameterWire {
    quantity: QuantityDefinition,
    value: Estimate<QuantityValue>,
}

impl<'de> Deserialize<'de> for RelationParameter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = RelationParameterWire::deserialize(deserializer)?;
        Ok(Self {
            quantity: value.quantity,
            value: value.value,
        })
    }
}

impl<'de> Deserialize<'de> for StateRelation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = StateRelationWire::deserialize(deserializer)?;
        Self::new(value.source, value.parameters).map_err(de::Error::custom)
    }
}

/// Failures which prevent a node equation from being stored.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum StateRelationError {
    /// A relation must compute something.
    #[error("a state relation requires a non-empty source")]
    EmptySource,
    /// The authored source exceeds the persisted source limit.
    #[error("state relation source exceeds the maximum accepted size")]
    SourceTooLarge,
    /// A parameter name cannot be bound as a Squiggle identifier.
    #[error("relation parameter '{0}' is not a valid binding name")]
    InvalidParameterName(String),
    /// A parameter name collides with one the generated prelude owns.
    #[error("relation parameter '{0}' shadows a generated binding")]
    ReservedParameterName(String),
    /// A parameter's value falls outside the support its quantity declares.
    #[error("relation parameter '{0}' falls outside its declared support")]
    ParameterOutsideSupport(String),
    /// The parameter's quantity definition is itself invalid.
    #[error(transparent)]
    Quantity(#[from] QuantityError),
    /// The source failed to parse or unit-check against the graph.
    #[error(transparent)]
    Relation(#[from] RelationError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Distribution, EstimateId, QuantitySupport};

    fn parameter(value: f64, support: QuantitySupport) -> RelationParameter {
        RelationParameter {
            quantity: QuantityDefinition::new("ratio", None, support).unwrap(),
            value: Estimate::new(EstimateId::new(0), Distribution::point(value).unwrap()).unwrap(),
        }
    }

    #[test]
    fn trims_source_and_round_trips_through_json() {
        let relation = StateRelation::new(
            "  outage_frequency * impact_duration  ".to_owned(),
            BTreeMap::from([(
                "suppression".to_owned(),
                parameter(0.4, QuantitySupport::Real),
            )]),
        )
        .unwrap();
        assert_eq!(relation.source, "outage_frequency * impact_duration");
        let json = serde_json::to_value(&relation).unwrap();
        assert_eq!(
            serde_json::from_value::<StateRelation>(json).unwrap(),
            relation
        );
    }

    #[test]
    fn rejects_empty_sources_and_unusable_parameter_names() {
        assert_eq!(
            StateRelation::new("   ".to_owned(), BTreeMap::new()),
            Err(StateRelationError::EmptySource)
        );
        for name in ["1st", "has space", "has-dash", ""] {
            assert!(matches!(
                StateRelation::new(
                    "baseline".to_owned(),
                    BTreeMap::from([(name.to_owned(), parameter(1.0, QuantitySupport::Real))]),
                ),
                Err(StateRelationError::InvalidParameterName(_))
            ));
        }
    }

    #[test]
    fn rejects_parameters_shadowing_the_generated_prelude() {
        assert_eq!(
            StateRelation::new(
                "baseline".to_owned(),
                BTreeMap::from([("baseline".to_owned(), parameter(1.0, QuantitySupport::Real))]),
            ),
            Err(StateRelationError::ReservedParameterName(
                "baseline".to_owned()
            ))
        );
    }

    #[test]
    fn rejects_a_parameter_outside_its_declared_support() {
        assert_eq!(
            StateRelation::new(
                "baseline".to_owned(),
                BTreeMap::from([(
                    "rate".to_owned(),
                    parameter(-1.0, QuantitySupport::NonNegative)
                )]),
            ),
            Err(StateRelationError::ParameterOutsideSupport(
                "rate".to_owned()
            ))
        );
    }
}
