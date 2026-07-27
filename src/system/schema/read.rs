//! Reading a design from a directory.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use super::{ComponentDocument, SCHEMA_VERSION, SchemaError, SystemDocument};

use crate::system::{
    catalogue::{builtin_catalogue, builtin_mutators},
    manifest::ComponentType,
    model::{Relationship, SystemModel},
    mutator::Mutator,
};

/// A design and the definitions needed to solve it.
#[derive(Clone, Debug)]
pub struct LoadedSystem {
    /// Human-readable name for the design.
    pub name: String,
    /// What the design is for.
    pub summary: String,
    /// The design itself.
    pub model: SystemModel,
    /// Shipped component types, with any the project defines for itself.
    pub component_types: BTreeMap<String, ComponentType>,
    /// Shipped behaviours, with any the project defines for itself.
    pub mutators: BTreeMap<String, Mutator>,
}

/// Reads a design from `directory`.
///
/// Project-local definitions are loaded over the shipped catalogue, so a design
/// may replace a component type it disagrees with as well as add one nobody
/// anticipated. They are validated by identical rules; nothing shipped is
/// privileged.
pub fn read_system(directory: &Path) -> Result<LoadedSystem, SchemaError> {
    let document: SystemDocument = parse(&directory.join("_system.yaml"))?;
    if document.schema_version != SCHEMA_VERSION {
        return Err(SchemaError::UnsupportedVersion {
            found: document.schema_version,
        });
    }

    let mut components = Vec::new();
    let mut relationships = Vec::new();
    for path in yaml_files(&directory.join("components"))? {
        let document: ComponentDocument = parse(&path)?;
        super::file_stem(document.component.id.as_str())?;
        for outgoing in document.outgoing {
            relationships.push(Relationship {
                from: document.component.id.clone(),
                to: outgoing.to,
                mutators: outgoing.mutators,
                summary: outgoing.summary,
            });
        }
        components.push(document.component);
    }
    if let Some(id) = super::duplicate(components.iter().map(|component| component.id.as_str())) {
        return Err(SchemaError::Duplicate { value: id });
    }

    let model = SystemModel {
        scratchpad: document.scratchpad,
        components,
        relationships,
        scale_units: document.scale_units,
        interventions: document.interventions,
    }
    .canonicalise();
    let known = model.identifiers();
    for relationship in &model.relationships {
        if !known.contains(&relationship.to) {
            return Err(SchemaError::DanglingRelationship {
                from: relationship.from.to_string(),
                to: relationship.to.to_string(),
            });
        }
    }

    let mut component_types = builtin_catalogue().map_err(|error| SchemaError::Definition {
        path: "<catalogue>".to_owned(),
        message: error.to_string(),
    })?;
    for path in yaml_files(&directory.join("component-types"))? {
        let definition: ComponentType = parse(&path)?;
        definition
            .validate()
            .map_err(|error| SchemaError::Definition {
                path: path.display().to_string(),
                message: error.to_string(),
            })?;
        component_types.insert(definition.id.to_string(), definition);
    }

    let mut mutators = builtin_mutators().map_err(|error| SchemaError::Definition {
        path: "<catalogue>".to_owned(),
        message: error.to_string(),
    })?;
    for path in yaml_files(&directory.join("mutators"))? {
        let definition: Mutator = parse(&path)?;
        definition
            .validate()
            .map_err(|error| SchemaError::Definition {
                path: path.display().to_string(),
                message: error.to_string(),
            })?;
        mutators.insert(definition.id.to_string(), definition);
    }

    Ok(LoadedSystem {
        name: document.name,
        summary: document.summary,
        model,
        component_types,
        mutators,
    })
}

fn parse<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, SchemaError> {
    let source = fs::read_to_string(path).map_err(|source| SchemaError::Io {
        path: path.display().to_string(),
        source,
    })?;
    serde_yaml_ng::from_str(&source).map_err(|error| SchemaError::Malformed {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

/// Lists the YAML documents directly inside a directory, in a stable order.
///
/// A missing directory is empty rather than an error, because the optional parts
/// of a design are absent from most of them. Only regular files one level down
/// are read, so a symbolic link to somewhere else cannot smuggle a document in.
fn yaml_files(directory: &Path) -> Result<Vec<PathBuf>, SchemaError> {
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(directory).map_err(|source| SchemaError::Io {
        path: directory.display().to_string(),
        source,
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| SchemaError::Io {
            path: directory.display().to_string(),
            source,
        })?;
        let kind = entry.file_type().map_err(|source| SchemaError::Io {
            path: entry.path().display().to_string(),
            source,
        })?;
        let path = entry.path();
        if kind.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "yaml")
        {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}
