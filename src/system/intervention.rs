//! Changes to a design, expressed as rebindings of shared quantities.
//!
//! # Why an intervention is not an edit
//!
//! Comparing two designs only means something if the two are otherwise
//! identical. Editing a model to try an idea destroys that guarantee: the
//! before and after differ by whatever was changed plus whatever was disturbed
//! along the way, and the difference in the result cannot be attributed to
//! either.
//!
//! An intervention therefore changes nothing structural. It rebinds named
//! quantities in the scratchpad and the model is solved again exactly as it
//! stands. Whatever moves in the result moved because of the rebinding, because
//! nothing else could have moved it.
//!
//! That constraint is also a design discipline. Expressing an idea as an
//! intervention forces the quantity it acts on to have been named in the first
//! place, which is usually where the thinking is. "Add a cache" is not a
//! proposal until it becomes "the hit ratio becomes 0.9", and the second is
//! something an engineer can argue with.
//!
//! # Reach
//!
//! Scratchpad entries may refer to earlier ones, so rebinding an early quantity
//! carries through everything derived from it. Rebinding a request rate moves
//! every component that sized itself against it, without any of them being
//! mentioned.
//!
//! # Rollout
//!
//! A replacement is an ordinary expression and may depend on time, so a change
//! that arrives gradually is written as one:
//! `if t < 300 then 100 else 400` deploys at five minutes. This is why the
//! shape of a rollout needs no separate machinery: the quantity was always a
//! function, and a constant was only the simplest case.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A stable identifier for an intervention within one model.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(transparent)]
pub struct InterventionId(String);

impl InterventionId {
    /// Creates an identifier from its text.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrows the identifier's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for InterventionId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// One shared quantity rebound by an intervention.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Override {
    /// Scratchpad entry being rebound.
    pub name: String,
    /// Squiggle source replacing the entry's own.
    pub expression: String,
}

/// A proposed change to a design.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Intervention {
    /// Identifier unique within the model.
    pub id: InterventionId,
    /// Human-readable name.
    pub name: String,
    /// What the change is and what it would cost to make.
    #[serde(default)]
    pub summary: String,
    /// Quantities this change rebinds.
    #[serde(default)]
    pub overrides: Vec<Override>,
}

impl Intervention {
    /// Returns the replacements keyed by the quantity they rebind.
    pub(super) fn bindings(&self) -> BTreeMap<String, String> {
        self.overrides
            .iter()
            .map(|entry| (entry.name.clone(), entry.expression.clone()))
            .collect()
    }
}
