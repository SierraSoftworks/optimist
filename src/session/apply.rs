//! Applying one change to a design.
//!
//! Every change is checked against the invariants the schema and the solver
//! rely on, and refused rather than half-applied. What is *not* checked is
//! completeness: a component missing a property, or a shared quantity nothing
//! refers to yet, is what the middle of an edit looks like and reporting it as
//! an error would make the tool unusable while someone was thinking.

use crate::system::SystemModel;

use super::mutation::{Mutation, MutationError};

pub(super) fn apply(model: &mut SystemModel, mutation: &Mutation) -> Result<(), MutationError> {
    match mutation {
        Mutation::SetScratchpadEntry { entry } => {
            replace(&mut model.scratchpad, entry.clone(), |existing| {
                existing.name == entry.name
            });
            Ok(())
        }
        Mutation::RemoveScratchpadEntry { name } => {
            remove(&mut model.scratchpad, |entry| &entry.name == name)
                .ok_or_else(|| absent("shared quantity", name))
        }
        Mutation::SetComponent { component } => {
            replace(&mut model.components, component.clone(), |existing| {
                existing.id == component.id
            });
            Ok(())
        }
        Mutation::RemoveComponent { id } => {
            remove(&mut model.components, |component| &component.id == id)
                .ok_or_else(|| absent("component", id.as_str()))?;
            // A connection to a component that is gone would make the design
            // unreadable, so removal takes its edges with it rather than
            // leaving the author to find them.
            model
                .relationships
                .retain(|relationship| &relationship.from != id && &relationship.to != id);
            for unit in &mut model.scale_units {
                unit.members.retain(|member| member != id);
            }
            Ok(())
        }
        Mutation::SetRelationship { relationship } => {
            if relationship.from == relationship.to {
                return Err(MutationError::SelfRelationship {
                    id: relationship.from.to_string(),
                });
            }
            let known = model.identifiers();
            for endpoint in [&relationship.from, &relationship.to] {
                if !known.contains(endpoint) {
                    return Err(MutationError::UnknownComponent {
                        id: endpoint.to_string(),
                    });
                }
            }
            replace(&mut model.relationships, relationship.clone(), |existing| {
                existing.from == relationship.from && existing.to == relationship.to
            });
            Ok(())
        }
        Mutation::RemoveRelationship { from, to } => remove(&mut model.relationships, |existing| {
            &existing.from == from && &existing.to == to
        })
        .ok_or_else(|| absent("connection", &format!("{from} to {to}"))),
        Mutation::SetScaleUnit { scale_unit } => {
            let known = model.identifiers();
            for member in &scale_unit.members {
                if !known.contains(member) {
                    return Err(MutationError::UnknownComponent {
                        id: member.to_string(),
                    });
                }
                let claimed = model
                    .scale_units
                    .iter()
                    .any(|unit| unit.id != scale_unit.id && unit.members.contains(member));
                if claimed {
                    return Err(MutationError::SharedMembership {
                        id: member.to_string(),
                    });
                }
            }
            if let Some(parent) = &scale_unit.parent {
                let resolves = parent != &scale_unit.id
                    && model.scale_units.iter().any(|unit| &unit.id == parent);
                if !resolves {
                    return Err(MutationError::UnknownScaleUnit {
                        id: parent.to_string(),
                    });
                }
            }
            replace(&mut model.scale_units, scale_unit.clone(), |existing| {
                existing.id == scale_unit.id
            });
            Ok(())
        }
        Mutation::RemoveScaleUnit { id } => {
            remove(&mut model.scale_units, |unit| &unit.id == id)
                .ok_or_else(|| absent("scale unit", id.as_str()))?;
            // A unit that enclosed the removed one becomes a root rather than
            // pointing at something that is gone.
            for unit in &mut model.scale_units {
                if unit.parent.as_ref() == Some(id) {
                    unit.parent = None;
                }
            }
            Ok(())
        }
        Mutation::SetIntervention { intervention } => {
            replace(&mut model.interventions, intervention.clone(), |existing| {
                existing.id == intervention.id
            });
            Ok(())
        }
        Mutation::RemoveIntervention { id } => remove(&mut model.interventions, |intervention| {
            &intervention.id == id
        })
        .ok_or_else(|| absent("intervention", id.as_str())),
    }
}

fn replace<T>(items: &mut Vec<T>, value: T, matches: impl Fn(&T) -> bool) {
    match items.iter_mut().find(|item| matches(item)) {
        Some(existing) => *existing = value,
        None => items.push(value),
    }
}

fn remove<T>(items: &mut Vec<T>, matches: impl Fn(&T) -> bool) -> Option<()> {
    let index = items.iter().position(matches)?;
    items.remove(index);
    Some(())
}

fn absent(what: &str, which: &str) -> MutationError {
    MutationError::Absent {
        what: format!("{what} '{which}'"),
    }
}
