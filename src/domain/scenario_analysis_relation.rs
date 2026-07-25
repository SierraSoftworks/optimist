use std::collections::{BTreeMap, BTreeSet};

use rand_chacha::ChaCha20Rng;

use super::{
    Distribution, RelationBindings, RelationProgram, ScenarioAnalysisError, StateRelation,
};
use crate::squiggle::Runtime;

/// One state's node equation, compiled once and evaluated every period.
///
/// Parents and activations are recorded so every declared binding can be given a
/// value even when this scenario never moves it. A parent nothing touches holds
/// its baseline and an intervention outside the candidate plan contributes no
/// activation, which keeps the equation total rather than failing on a name the
/// author can see in the graph.
#[derive(Clone)]
pub(super) struct CompiledRelation {
    program: RelationProgram,
    parents: BTreeMap<String, usize>,
    activations: BTreeSet<String>,
    parameters: Vec<(String, Distribution)>,
}

impl CompiledRelation {
    pub(super) fn new(
        program: RelationProgram,
        parents: BTreeMap<String, usize>,
        activations: BTreeSet<String>,
        relation: &StateRelation,
    ) -> Self {
        Self {
            program,
            parents,
            activations,
            parameters: relation
                .parameters
                .iter()
                .map(|(name, parameter)| (name.clone(), parameter.value.distribution.clone()))
                .collect(),
        }
    }

    /// Draws this relation's coefficients once for a whole Monte Carlo iteration.
    ///
    /// A coefficient is a property of the world rather than of a period, so it is
    /// sampled with the draw and held constant across the horizon. Resampling it
    /// per period would model it as noise and quietly average the equation's
    /// response away.
    pub(super) fn sample_parameters(&self, rng: &mut ChaCha20Rng) -> BTreeMap<String, f64> {
        self.parameters
            .iter()
            .map(|(name, distribution)| (name.clone(), distribution.sample(rng)))
            .collect()
    }

    /// Binds every declared name to its default for one period.
    pub(super) fn bindings(
        &self,
        baseline: f64,
        baselines: &[f64],
        parameters: &BTreeMap<String, f64>,
    ) -> RelationBindings {
        RelationBindings {
            baseline,
            parents: self
                .parents
                .iter()
                .map(|(name, index)| (name.clone(), baselines[*index]))
                .collect(),
            parameters: parameters.clone(),
            activations: self
                .activations
                .iter()
                .map(|name| (name.clone(), 0.0))
                .collect(),
        }
    }

    /// Reports which states this equation reads, so baselines can settle in order.
    pub(super) fn parent_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.parents.values().copied()
    }

    /// Records a parent's value for this period, ignoring names it cannot bind.
    ///
    /// A relationship may reach a state that has no relation, or reach one from a
    /// parent the equation never declared; either way the value is simply not
    /// part of this equation.
    pub(super) fn set_parent(&self, bindings: &mut RelationBindings, name: &str, value: f64) {
        if let Some(slot) = bindings.parents.get_mut(name) {
            *slot = value;
        }
    }

    /// Records an intervention's activation for this period.
    pub(super) fn set_activation(&self, bindings: &mut RelationBindings, name: &str, value: f64) {
        if let Some(slot) = bindings.activations.get_mut(name) {
            *slot = value;
        }
    }

    pub(super) fn evaluate(
        &self,
        runtime: &mut Runtime,
        bindings: &RelationBindings,
    ) -> Result<f64, ScenarioAnalysisError> {
        self.program
            .evaluate(runtime, bindings)
            .map_err(|error| ScenarioAnalysisError::Relation(error.to_string()))
    }
}
