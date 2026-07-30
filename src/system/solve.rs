//! Asking a model a question, and being told how the answering is going.
//!
//! # Why a builder
//!
//! A solve is parameterised by more than its configuration. It reads a
//! catalogue of component types, it may read behaviours the shipped catalogue
//! never anticipated, it may have an intervention applied, and it may want to
//! say how it is getting on while it runs. Expressed as free functions those
//! combine multiplicatively, which is how this crate came to have a
//! `_with_mutators` suffix on half its entry points and no room for a fifth
//! thing to vary.
//!
//! Everything is borrowed, which keeps [`EvaluationConfig`] a plain `Copy` value
//! that a caller can hold, compare and hash without a lifetime attached to it.

use std::{borrow::Cow, collections::BTreeMap};

use super::{
    Bottleneck,
    comparison::Comparison,
    evaluate::{
        Evaluation, EvaluationConfig, EvaluationError,
        progress::{Progress, Reporting},
    },
    intervention::InterventionId,
    manifest::ComponentType,
    model::SystemModel,
    mutator::Mutator,
};

/// A question to put to a model.
///
/// ```
/// use optimist::system::{EvaluationConfig, InterventionId, Solve, SystemModel, builtin_catalogue};
///
/// let model: SystemModel = serde_yaml_ng::from_str("
/// scratchpad:
///   - name: peak_rate
///     expression: '900'
/// components:
///   - id: users
///     name: Users
///     type: client
///     properties:
///       request_rate: peak_rate
///   - id: api
///     name: API
///     type: compute
///     properties:
///       service_time: '0.02'
///       parallelism: '8'
/// relationships:
///   - from: users
///     to: api
/// interventions:
///   - id: quieter
///     name: Shift traffic away
///     overrides:
///       - name: peak_rate
///         expression: '200'
/// ")?;
///
/// let catalogue = builtin_catalogue()?;
/// let quieter = InterventionId::new("quieter");
/// let asking = Solve::new(&model, &catalogue).with(EvaluationConfig::default());
///
/// let loaded = asking.evaluate()?;
/// let relieved = asking.intervention(&quieter).evaluate()?;
///
/// // Eight slots at 20 ms sustain 400 per second, so the quieter design settles.
/// assert!(loaded.converged() && relieved.converged());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy)]
pub struct Solve<'a> {
    model: &'a SystemModel,
    catalogue: &'a BTreeMap<String, ComponentType>,
    mutators: Option<&'a BTreeMap<String, Mutator>>,
    scenario: Scenario<'a>,
    config: EvaluationConfig,
    progress: Option<&'a dyn Progress>,
}

/// Which version of the design is being solved.
#[derive(Clone, Copy)]
enum Scenario<'a> {
    Unchanged,
    Applying(&'a InterventionId),
    Rebinding(&'a BTreeMap<String, String>),
}

impl<'a> Solve<'a> {
    /// Prepares to solve a model against a catalogue of component types.
    pub fn new(model: &'a SystemModel, catalogue: &'a BTreeMap<String, ComponentType>) -> Self {
        Self {
            model,
            catalogue,
            mutators: None,
            scenario: Scenario::Unchanged,
            config: EvaluationConfig::default(),
            progress: None,
        }
    }

    /// Sets how the model should be solved.
    #[must_use]
    pub fn with(self, config: EvaluationConfig) -> Self {
        Self { config, ..self }
    }

    /// Supplies the relationship behaviours to read, in place of the shipped set.
    ///
    /// A design may attach behaviours the shipped catalogue never anticipated,
    /// and solving without them silently drops the rewrites they apply to the
    /// flows travelling along a relationship.
    #[must_use]
    pub fn mutators(self, mutators: &'a BTreeMap<String, Mutator>) -> Self {
        Self {
            mutators: Some(mutators),
            ..self
        }
    }

    /// Applies one of the model's interventions.
    ///
    /// Read by [`evaluate`](Self::evaluate) and [`bottlenecks`](Self::bottlenecks).
    /// A comparison defines its own pair of scenarios, so this has no bearing on
    /// [`compare`](Self::compare).
    #[must_use]
    pub fn intervention(self, intervention: &'a InterventionId) -> Self {
        Self {
            scenario: Scenario::Applying(intervention),
            ..self
        }
    }

    /// Rebinds shared quantities directly, without naming an intervention.
    ///
    /// This is what an intervention resolves to, and it is exposed for callers
    /// weighing a change that is not written into the design.
    #[must_use]
    pub fn overrides(self, overrides: &'a BTreeMap<String, String>) -> Self {
        Self {
            scenario: Scenario::Rebinding(overrides),
            ..self
        }
    }

    /// Says where to report progress while the solve runs.
    ///
    /// Nothing is reported by default, and a solve nobody is watching costs a
    /// branch per pass rather than a call.
    #[must_use]
    pub fn reporting(self, progress: &'a dyn Progress) -> Self {
        Self {
            progress: Some(progress),
            ..self
        }
    }

    /// Solves the model, relaxing each step of the horizon toward its fixed point.
    pub fn evaluate(&self) -> Result<Evaluation, EvaluationError> {
        let mutators = self.behaviours();
        let unchanged = BTreeMap::new();
        let applied;
        let overrides = match self.scenario {
            Scenario::Unchanged => &unchanged,
            Scenario::Applying(intervention) => {
                applied = self.model.intervention(intervention)?.bindings();
                &applied
            }
            Scenario::Rebinding(overrides) => overrides,
        };
        super::evaluate::solved(
            self.model,
            self.catalogue,
            &mutators,
            overrides,
            self.config,
            Reporting::to(self.progress),
        )
    }

    /// Weighs one of the model's interventions against the design as it stands.
    ///
    /// The two scenarios are solved side by side, each reporting under its own
    /// [`Job`](super::progress::Job), so a caller drawing one bar per solve can
    /// tell which of them is holding the answer up.
    pub fn compare(&self, intervention: &InterventionId) -> Result<Comparison, EvaluationError> {
        super::comparison::compared(
            self.model,
            self.catalogue,
            &self.behaviours(),
            intervention,
            self.config,
            Reporting::to(self.progress),
        )
    }

    /// Weighs several proposals against one shared baseline.
    ///
    /// The unchanged design does not depend on which proposal it is being
    /// weighed against, so it is solved once and shared: `n` proposals cost
    /// `n + 1` solves rather than `2n`, and every comparison reads the same
    /// baseline computed with the same seed.
    ///
    /// ```
    /// use optimist::system::{EvaluationConfig, InterventionId, Solve, SystemModel, builtin_catalogue};
    ///
    /// let model: SystemModel = serde_yaml_ng::from_str("
    /// scratchpad:
    ///   - name: peak_rate
    ///     expression: '900'
    /// components:
    ///   - id: users
    ///     name: Users
    ///     type: client
    ///     properties:
    ///       request_rate: peak_rate
    ///   - id: api
    ///     name: API
    ///     type: compute
    ///     properties:
    ///       service_time: '0.02'
    ///       parallelism: '8'
    /// relationships:
    ///   - from: users
    ///     to: api
    /// interventions:
    ///   - id: quieter
    ///     name: Shift traffic away
    ///     overrides:
    ///       - name: peak_rate
    ///         expression: '200'
    ///   - id: bigger
    ///     name: Add workers
    ///     overrides:
    ///       - name: peak_rate
    ///         expression: '400'
    /// ")?;
    ///
    /// let weighed = Solve::new(&model, &builtin_catalogue()?)
    ///     .with(EvaluationConfig::default())
    ///     .compare_many(&[InterventionId::new("quieter"), InterventionId::new("bigger")])?;
    ///
    /// assert_eq!(weighed.len(), 2);
    /// // Both proposals were weighed against the very same ranking of the design.
    /// assert_eq!(
    ///     weighed[0].1.baseline[0].utilisation,
    ///     weighed[1].1.baseline[0].utilisation,
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn compare_many(
        &self,
        interventions: &[InterventionId],
    ) -> Result<Vec<(InterventionId, Comparison)>, EvaluationError> {
        super::comparison::compared_many(
            self.model,
            self.catalogue,
            &self.behaviours(),
            interventions,
            self.config,
            Reporting::to(self.progress),
        )
    }

    /// Ranks what a solved step is closest to exhausting, worst first.
    pub fn bottlenecks(&self, step: &super::Step) -> Result<Vec<Bottleneck>, EvaluationError> {
        super::bottlenecks_with_mutators(
            self.model,
            self.catalogue,
            &self.behaviours(),
            step,
            self.config,
        )
    }

    fn behaviours(&self) -> Cow<'a, BTreeMap<String, Mutator>> {
        match self.mutators {
            Some(mutators) => Cow::Borrowed(mutators),
            None => Cow::Owned(super::evaluate::builtin_mutators_or_empty()),
        }
    }
}
