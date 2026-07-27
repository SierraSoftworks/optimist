//! The component types shipped with the tool.
//!
//! The catalogue is embedded in the binary and validated on load, so a
//! malformed builtin definition fails a test rather than a user's model. Types
//! defined by a project are loaded alongside these and are checked by exactly
//! the same rules; nothing here is privileged.
//!
//! The vocabulary is deliberately small. Each entry covers a role that recurs in
//! nearly every system design, and anything more specialised is better expressed
//! as a project-local type than as a catalogue entry nobody else can use.

use std::{collections::BTreeMap, fmt};

use super::{
    manifest::{ComponentType, ComponentTypeId},
    validate::ComponentTypeError,
};

const MANIFESTS: &[&str] = &[
    include_str!("catalogue/client.yaml"),
    include_str!("catalogue/load-balancer.yaml"),
    include_str!("catalogue/queue.yaml"),
    include_str!("catalogue/compute.yaml"),
    include_str!("catalogue/datastore.yaml"),
    include_str!("catalogue/aggregator.yaml"),
];

/// Why a catalogue could not be assembled.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CatalogueError {
    /// A manifest is not well-formed YAML or omits a required field.
    Malformed {
        /// The parser's description of the problem.
        message: String,
    },
    /// A manifest describes a component type that cannot be used.
    Invalid {
        /// The offending type, where its identifier could be read.
        id: String,
        /// Why the type was rejected.
        source: ComponentTypeError,
    },
    /// Two manifests claim the same identifier.
    Duplicate {
        /// The contested identifier.
        id: ComponentTypeId,
    },
}

impl fmt::Display for CatalogueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { message } => write!(formatter, "malformed manifest: {message}"),
            Self::Invalid { id, source } => write!(formatter, "component type '{id}': {source}"),
            Self::Duplicate { id } => {
                write!(formatter, "component type '{id}' is defined more than once")
            }
        }
    }
}

impl std::error::Error for CatalogueError {}

/// Loads and validates the component types shipped with the tool.
///
/// ```
/// let catalogue = optimist::system::builtin_catalogue()?;
/// let compute = &catalogue["compute"];
/// assert!(compute.properties.contains_key("service_time"));
/// assert!(compute.constraints.contains_key("capacity"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn builtin_catalogue() -> Result<BTreeMap<String, ComponentType>, CatalogueError> {
    let mut catalogue = BTreeMap::new();
    for manifest in MANIFESTS {
        let component: ComponentType =
            serde_yaml_ng::from_str(manifest).map_err(|error| CatalogueError::Malformed {
                message: error.to_string(),
            })?;
        component
            .validate()
            .map_err(|source| CatalogueError::Invalid {
                id: component.id.to_string(),
                source,
            })?;
        if catalogue
            .insert(component.id.to_string(), component.clone())
            .is_some()
        {
            return Err(CatalogueError::Duplicate { id: component.id });
        }
    }
    Ok(catalogue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_type_loads_and_validates() {
        let catalogue = builtin_catalogue().expect("catalogue");
        assert_eq!(catalogue.len(), MANIFESTS.len());
    }

    #[test]
    fn the_expected_roles_are_covered() {
        let catalogue = builtin_catalogue().expect("catalogue");
        for id in [
            "client",
            "load-balancer",
            "queue",
            "compute",
            "datastore",
            "aggregator",
        ] {
            assert!(catalogue.contains_key(id), "missing '{id}'");
        }
    }

    #[test]
    fn every_type_documents_itself() {
        for component in builtin_catalogue().expect("catalogue").values() {
            assert!(!component.name.is_empty(), "{} has no name", component.id);
            assert!(
                !component.summary.trim().is_empty(),
                "{} has no summary",
                component.id
            );
            for (name, property) in &component.properties {
                assert!(
                    !property.summary.trim().is_empty(),
                    "{}.{name} has no summary",
                    component.id
                );
            }
        }
    }

    #[test]
    fn every_constraint_explains_what_saturating_it_means() {
        for component in builtin_catalogue().expect("catalogue").values() {
            for (name, constraint) in &component.constraints {
                assert!(
                    !constraint.summary.trim().is_empty(),
                    "{}.{name} has no summary",
                    component.id
                );
            }
        }
    }

    #[test]
    fn every_type_that_serves_demand_declares_a_constraint() {
        // A component with no limit can never be reported as a bottleneck, which
        // is only honest for a demand source or a pure transformation.
        let catalogue = builtin_catalogue().expect("catalogue");
        for id in ["queue", "compute", "datastore", "load-balancer"] {
            assert!(
                !catalogue[id].constraints.is_empty(),
                "'{id}' declares no constraint"
            );
        }
    }
}
