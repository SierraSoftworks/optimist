//! Groups of components that are replicated together.
//!
//! # Designing the unit, then counting units
//!
//! Large systems are not built by scaling every part independently. They are
//! built by designing a self-contained unit — a cell, a shard, a zone, a region
//! — and then deploying many of them. A scale unit names that boundary, so a
//! model describes one unit and says how many exist rather than describing the
//! whole fleet at once.
//!
//! Constraints are therefore evaluated per unit, which is the question worth
//! asking. "Does one cell have enough capacity" has an answer an engineer can
//! act on; "does the fleet have enough capacity in total" hides the cell that
//! is hot while the average looks fine.
//!
//! # Nesting
//!
//! Units nest, because real deployments do: a region contains cells, a cell
//! contains shards. A component's effective replica count is the product along
//! its chain of enclosing units, so a component inside ten shards inside three
//! regions is deployed thirty times.
//!
//! # Distribution
//!
//! How demand meets those replicas is a modelling decision, not a consequence of
//! the count. Sharded traffic divides: each replica serves its share. Mirrored
//! traffic does not: replicating writes to every region means every region sees
//! every write, so the count multiplies cost without dividing load. Confusing
//! the two is how a design ends up sized for a fraction of its real demand.
//!
//! # Against a component's own replica count
//!
//! A component type may declare its own replica property, and that is a
//! different statement. It replicates *one* component behind a shared entry
//! point, where a scale unit replicates a *set* of components together as a
//! deployable whole. A pool of servers is the former; a cell containing a pool,
//! its queue, and its store is the latter.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::model::ComponentId;

/// A stable identifier for a scale unit within one model.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(transparent)]
pub struct ScaleUnitId(String);

impl ScaleUnitId {
    /// Creates an identifier from its text.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrows the identifier's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ScaleUnitId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// How demand is spread across the replicas of a scale unit.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Distribution {
    /// Each replica serves its share, as a sharded or load-balanced fleet does.
    #[default]
    Sharded,
    /// Every replica sees the whole flow, as with writes replicated everywhere.
    Mirrored,
}

/// A set of components deployed together as one replicated whole.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScaleUnit {
    /// Identifier unique within the model.
    pub id: ScaleUnitId,
    /// Human-readable name.
    pub name: String,
    /// What this boundary represents.
    #[serde(default)]
    pub summary: String,
    /// Squiggle source for how many replicas exist.
    pub replicas: String,
    /// How demand is spread across those replicas.
    #[serde(default)]
    pub distribution: Distribution,
    /// Components deployed inside this unit.
    #[serde(default)]
    pub members: Vec<ComponentId>,
    /// The unit enclosing this one, where it is itself replicated.
    #[serde(default)]
    pub parent: Option<ScaleUnitId>,
}

/// Resolves the chain of units enclosing each component.
///
/// Returns each component's units from innermost outward, which is the order in
/// which their replica counts multiply.
pub(super) fn enclosing(units: &[ScaleUnit]) -> BTreeMap<ComponentId, Vec<ScaleUnitId>> {
    let parents = units
        .iter()
        .map(|unit| (unit.id.clone(), unit.parent.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut chains = BTreeMap::new();
    for unit in units {
        for member in &unit.members {
            let mut chain = vec![unit.id.clone()];
            let mut current = unit.parent.clone();
            // The nesting graph is checked acyclic before this runs, so the
            // bound only guards against a caller skipping validation.
            while let Some(parent) = current {
                if chain.contains(&parent) {
                    break;
                }
                current = parents.get(&parent).cloned().flatten();
                chain.push(parent);
            }
            chains.insert(member.clone(), chain);
        }
    }
    chains
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(id: &str, parent: Option<&str>, members: &[&str]) -> ScaleUnit {
        ScaleUnit {
            id: ScaleUnitId::new(id),
            name: id.to_owned(),
            summary: String::new(),
            replicas: "1".to_owned(),
            distribution: Distribution::Sharded,
            members: members.iter().map(|id| ComponentId::new(*id)).collect(),
            parent: parent.map(ScaleUnitId::new),
        }
    }

    #[test]
    fn a_member_reports_its_own_unit() {
        let chains = enclosing(&[unit("cell", None, &["api"])]);
        assert_eq!(
            chains[&ComponentId::new("api")],
            vec![ScaleUnitId::new("cell")]
        );
    }

    #[test]
    fn nesting_walks_outward_from_the_member() {
        let chains = enclosing(&[
            unit("region", None, &[]),
            unit("cell", Some("region"), &["api"]),
        ]);
        assert_eq!(
            chains[&ComponentId::new("api")],
            vec![ScaleUnitId::new("cell"), ScaleUnitId::new("region")]
        );
    }

    #[test]
    fn a_component_outside_every_unit_has_no_chain() {
        let chains = enclosing(&[unit("cell", None, &["api"])]);
        assert!(!chains.contains_key(&ComponentId::new("users")));
    }
}
