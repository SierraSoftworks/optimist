use super::scenario_analysis_state::{Combination, StateNode};

/// One period's accumulated response for every state, before clamping.
///
/// Each state accumulates in whichever space its combination rule uses, so a
/// multiplicative state gathers a product starting from one while an additive
/// state gathers a fractional sum starting from zero.
pub(super) struct Accumulator {
    values: Vec<f64>,
    /// Responses discarded because their source had no usable ratio scale.
    pub(super) undefined: u64,
}

impl Accumulator {
    pub(super) fn new(states: &[StateNode]) -> Self {
        Self {
            values: states
                .iter()
                .map(|state| match state.combination {
                    Combination::Multiplicative => 1.0,
                    Combination::Additive => 0.0,
                })
                .collect(),
            undefined: 0,
        }
    }

    /// Applies one proportional response of `ratio` at `weight` in `[0, 1]`.
    ///
    /// Weight is the temporal activation of an intervention effect, so a fully
    /// active multiplier applies as itself and an inactive one as no change. In
    /// multiplicative space that is $\text{ratio}^{\text{weight}}$, which
    /// interpolates geometrically while a profile ramps or releases; in additive
    /// space it is the linear share $(\text{ratio}-1)\cdot\text{weight}$.
    pub(super) fn multiplier(&mut self, state: &StateNode, index: usize, ratio: f64, weight: f64) {
        match state.combination {
            Combination::Multiplicative => self.values[index] *= ratio.powf(weight),
            Combination::Additive => self.values[index] += (ratio - 1.0) * weight,
        }
    }

    /// Applies an elasticity against a source's fractional movement.
    ///
    /// A source with a zero or non-finite baseline has no ratio scale, so its
    /// fractional movement is undefined. Rather than propagating an infinity, the
    /// response is dropped and counted, which surfaces a modelling problem as a
    /// diagnostic instead of a failed analysis.
    pub(super) fn elasticity(
        &mut self,
        state: &StateNode,
        index: usize,
        elasticity: f64,
        source: f64,
        source_baseline: f64,
    ) {
        if source_baseline == 0.0 || !source_baseline.is_finite() {
            self.undefined = self.undefined.saturating_add(1);
            return;
        }
        let ratio = source / source_baseline;
        match state.combination {
            Combination::Multiplicative => self.values[index] *= ratio.powf(elasticity),
            Combination::Additive => self.values[index] += elasticity * (ratio - 1.0),
        }
    }

    /// Resolves each state's level for this period from its sampled baseline.
    pub(super) fn resolve(&self, states: &[StateNode], baselines: &[f64]) -> Vec<f64> {
        states
            .iter()
            .enumerate()
            .map(|(index, state)| match state.combination {
                Combination::Multiplicative => baselines[index] * self.values[index],
                Combination::Additive => baselines[index] * (1.0 + self.values[index]),
            })
            .collect()
    }
}
