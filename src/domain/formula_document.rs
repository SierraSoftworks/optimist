use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{CompiledFormula, EstimateAddress, Formula};

/// One stored Fermi component formula and its derived validation metadata.
///
/// `compiled` is recomputed during project mutation and retained so API clients
/// can inspect units and dependencies without reimplementing formula validation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FormulaDefinition {
    /// Nested component address defined by this formula.
    pub address: EstimateAddress,
    /// Dimension-aware expression persisted as the source of truth.
    pub formula: Formula,
    /// Derived unit and deterministic dependency order.
    pub compiled: CompiledFormula,
    /// Evidence or elicitation context for this decomposition.
    pub provenance: Vec<String>,
}

/// Current formula document revision and all compiled component definitions.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FormulaCatalog {
    /// Formula document revision used by set and remove commands.
    pub revision: u64,
    /// Definitions ordered by canonical component address.
    pub formulas: Vec<FormulaDefinition>,
}

/// Revisioned project-scoped set of Fermi component definitions.
///
/// Entries are ordered by canonical estimate address. Formula definitions live
/// outside the causal graph because they may reference estimates across multiple
/// graph aggregates while retaining project isolation.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct FormulaDocument {
    /// Revision incremented by every successful set or removal command.
    pub revision: u64,
    /// Formula sources indexed by their nested component address.
    pub formulas: BTreeMap<EstimateAddress, Formula>,
    /// Per-formula evidence or elicitation records under the same addresses.
    #[serde(default)]
    pub provenance: BTreeMap<EstimateAddress, Vec<String>>,
}
