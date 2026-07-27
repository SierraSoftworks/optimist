//! Writing a design to a directory.

use std::{fs, path::Path};

use super::{ComponentDocument, OutgoingRelationship, SCHEMA_VERSION, SchemaError, SystemDocument};

use crate::system::model::SystemModel;

/// Writes a design to `directory`, creating it if necessary.
///
/// Relationships are stored with the component they leave, so the same model
/// always produces the same files and a change to one component touches one
/// file. Existing component documents that the model no longer contains are
/// removed, because a stale file would be read back as a component nobody
/// declared.
pub fn write_system(
    directory: &Path,
    name: &str,
    summary: &str,
    model: &SystemModel,
) -> Result<(), SchemaError> {
    if let Some(id) = super::duplicate(
        model
            .components
            .iter()
            .map(|component| component.id.as_str()),
    ) {
        return Err(SchemaError::Duplicate { value: id });
    }
    let known = model.identifiers();
    for relationship in &model.relationships {
        if !known.contains(&relationship.to) {
            return Err(SchemaError::DanglingRelationship {
                from: relationship.from.to_string(),
                to: relationship.to.to_string(),
            });
        }
    }

    let components = directory.join("components");
    create(&components)?;
    write(
        &directory.join("_system.yaml"),
        &SystemDocument {
            schema_version: SCHEMA_VERSION,
            name: name.to_owned(),
            summary: summary.to_owned(),
            scratchpad: model.scratchpad.clone(),
            scale_units: model.scale_units.clone(),
            interventions: model.interventions.clone(),
        },
    )?;

    let mut written = Vec::new();
    for component in &model.components {
        let stem = super::file_stem(component.id.as_str())?;
        let document = ComponentDocument {
            component: component.clone(),
            outgoing: model
                .relationships
                .iter()
                .filter(|relationship| relationship.from == component.id)
                .map(|relationship| OutgoingRelationship {
                    to: relationship.to.clone(),
                    mutators: relationship.mutators.clone(),
                    summary: relationship.summary.clone(),
                })
                .collect(),
        };
        let file = format!("{stem}.yaml");
        write(&components.join(&file), &document)?;
        written.push(file);
    }
    prune(&components, &written)
}

fn create(directory: &Path) -> Result<(), SchemaError> {
    fs::create_dir_all(directory).map_err(|source| SchemaError::Io {
        path: directory.display().to_string(),
        source,
    })
}

fn write<T: serde::Serialize>(path: &Path, document: &T) -> Result<(), SchemaError> {
    let rendered = serde_yaml_ng::to_string(document).map_err(|error| SchemaError::Malformed {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    if let Some(parent) = path.parent() {
        create(parent)?;
    }
    fs::write(path, rendered).map_err(|source| SchemaError::Io {
        path: path.display().to_string(),
        source,
    })
}

/// Removes component documents the model no longer contains.
fn prune(directory: &Path, keep: &[String]) -> Result<(), SchemaError> {
    let entries = fs::read_dir(directory).map_err(|source| SchemaError::Io {
        path: directory.display().to_string(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| SchemaError::Io {
            path: directory.display().to_string(),
            source,
        })?;
        let path = entry.path();
        let stale = path
            .extension()
            .is_some_and(|extension| extension == "yaml")
            && !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| keep.iter().any(|kept| kept == name));
        if stale {
            fs::remove_file(&path).map_err(|source| SchemaError::Io {
                path: path.display().to_string(),
                source,
            })?;
        }
    }
    Ok(())
}
