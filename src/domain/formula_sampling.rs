use std::collections::BTreeMap;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use thiserror::Error;

use super::formula_dependence;
use super::formula_draw::{SampleFailure, draw};
use super::online_moments::OnlineJointMoments;
use super::{
    ConvergenceStatus, DependenceError, EstimateAddress, Formula, FormulaError, FormulaSet,
    InvalidSampleCounts, JointMonteCarloReport, MonteCarloConfig, MonteCarloDiagnostics,
    MonteCarloEstimate, ProjectDependenceModel, ProjectId,
};

/// Failures that prevent a Monte Carlo run from starting.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum MonteCarloError {
    /// Joint sampling requires at least one root formula.
    #[error("joint Monte Carlo sampling requires at least one root formula")]
    EmptyRoots,
    /// A formula failed project, cycle, arity, bounds, or unit validation.
    #[error(transparent)]
    Formula(#[from] FormulaError),
    /// The dependence document failed project, membership, or matrix validation.
    #[error(transparent)]
    Dependence(#[from] DependenceError),
    /// A dependence member is absent from the formula definitions.
    #[error("dependence member {0} is absent from the formula set")]
    MissingDependenceMember(EstimateAddress),
    /// A dependence member does not directly identify a primitive marginal.
    #[error("dependence member {0} must identify a literal marginal distribution")]
    NonMarginalDependenceMember(EstimateAddress),
}

impl FormulaSet {
    /// Samples formula roots jointly with one memoized draw per referenced address.
    ///
    /// All roots are validated for project isolation, cycles, dimensions, arity,
    /// and bounds before RNG state is consumed. Each valid draw uses a fresh address
    /// memo, so repeated references are identical within that draw and independent
    /// across draws. Literals are sampled whenever encountered; sharing requires an
    /// [`crate::domain::EstimateAddress`] reference. `Bounded` clamps finite values. A zero ratio
    /// denominator or any non-finite primitive/arithmetic result rejects the whole
    /// joint draw and is counted rather than replaced. Estimates use online Pébay/
    /// Welford central-moment recurrences, avoiding sample retention. See Pébay,
    /// Sandia report SAND2008-6212, equations 1.5-1.8.
    pub fn sample_joint(
        &self,
        project: &ProjectId,
        roots: &[Formula],
        config: MonteCarloConfig,
    ) -> Result<JointMonteCarloReport, MonteCarloError> {
        self.sample_joint_inner(project, roots, config, None)
    }

    /// Samples formula roots with project-level Gaussian copula dependence.
    ///
    /// For each group, $Z\sim N(0,R)$ is transformed to $U_i=\Phi(Z_i)$ and then
    /// to addressed marginals $X_i=F_i^{-1}(U_i)$. These values pre-populate the
    /// per-draw address memo, so every reference reuses its joint marginal draw.
    /// Members must identify literal formula definitions; composite formulas do
    /// not declare a marginal inverse CDF and are rejected. Point masses remain
    /// constant, and resulting ties can reduce empirical rank correlation. Uniforms
    /// are clamped to the representable open interval, truncating only extreme
    /// floating-point tails. See Nelsen, *An Introduction to Copulas*, section 5.1.
    pub fn sample_joint_with_dependence(
        &self,
        project: &ProjectId,
        roots: &[Formula],
        config: MonteCarloConfig,
        dependence: &ProjectDependenceModel,
    ) -> Result<JointMonteCarloReport, MonteCarloError> {
        self.sample_joint_inner(project, roots, config, Some(dependence))
    }

    fn sample_joint_inner(
        &self,
        project: &ProjectId,
        roots: &[Formula],
        config: MonteCarloConfig,
        dependence: Option<&ProjectDependenceModel>,
    ) -> Result<JointMonteCarloReport, MonteCarloError> {
        if roots.is_empty() {
            return Err(MonteCarloError::EmptyRoots);
        }
        for root in roots {
            self.validate(project, root)?;
        }
        if let Some(model) = dependence {
            model.validate_for_project(project)?;
            formula_dependence::validate(self, model)?;
        }
        let mut rng = ChaCha20Rng::seed_from_u64(config.seed());
        let mut moments = OnlineJointMoments::new(roots.len());
        let mut invalid = InvalidSampleCounts::default();
        let mut attempted = 0;
        while attempted < config.maximum_samples() {
            attempted += 1;
            let mut memo = BTreeMap::new();
            if let Some(model) = dependence {
                formula_dependence::populate(self, model, &mut rng, &mut memo);
            }
            let draw: Result<Vec<_>, _> = roots
                .iter()
                .map(|root| draw(self, root, &mut rng, &mut memo))
                .collect();
            match draw {
                Ok(values) => moments.push(&values),
                Err(SampleFailure::ZeroDenominator) => invalid.zero_denominator += 1,
                Err(SampleFailure::NonFinitePrimitive) => invalid.non_finite_primitive += 1,
                Err(SampleFailure::NonFiniteResult) => invalid.non_finite_result += 1,
            }
            if config.converged(&moments, roots.len()) {
                break;
            }
        }
        Ok(report(config, attempted, invalid, &moments, roots.len()))
    }
}

fn report(
    config: MonteCarloConfig,
    attempted: u64,
    invalid: InvalidSampleCounts,
    moments: &OnlineJointMoments,
    dimensions: usize,
) -> JointMonteCarloReport {
    let estimates = (0..dimensions)
        .map(|index| MonteCarloEstimate {
            mean: moments.mean(index),
            variance: moments.variance(index),
            mean_standard_error: moments.mean_standard_error(index),
            variance_standard_error: moments.variance_standard_error(index),
        })
        .collect();
    let covariance = (0..dimensions)
        .map(|row| {
            (0..dimensions)
                .map(|column| moments.covariance(row, column))
                .collect()
        })
        .collect();
    let status = if config.converged(moments, dimensions) {
        ConvergenceStatus::Converged
    } else if moments.count() < config.minimum_samples() {
        ConvergenceStatus::InsufficientValidSamples
    } else {
        ConvergenceStatus::MaximumSamplesReached
    };
    JointMonteCarloReport {
        estimates,
        covariance,
        diagnostics: MonteCarloDiagnostics {
            seed: config.seed(),
            attempted_samples: attempted,
            valid_samples: moments.count(),
            invalid_samples: invalid,
            criterion: config,
            status,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        CorrelationScale, Distribution, EntityId, EstimateAddress, EstimateId, EstimateOwner,
        GaussianCopulaCorrelation, ProjectDependenceModel, ResidualDependenceGroup, Unit,
    };

    fn project() -> ProjectId {
        ProjectId::new("sampling").unwrap()
    }
    fn address() -> EstimateAddress {
        EstimateAddress::new(
            project(),
            EstimateOwner::Node(EntityId::new(1)),
            EstimateId::new(1),
        )
    }
    fn literal(distribution: Distribution) -> Formula {
        Formula::Literal {
            distribution,
            unit: Unit::dimensionless(),
        }
    }
    fn config(seed: u64) -> MonteCarloConfig {
        MonteCarloConfig::new(seed, 2_000, 20_000, 0.002, 0.0).unwrap()
    }

    #[test]
    fn same_seed_is_bit_reproducible() {
        let root = literal(Distribution::normal(2.0, 3.0).unwrap());
        let first = FormulaSet::default()
            .sample_joint(&project(), std::slice::from_ref(&root), config(42))
            .unwrap();
        let second = FormulaSet::default()
            .sample_joint(&project(), &[root], config(42))
            .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn shared_references_are_one_draw_and_joint_covariance_matches() {
        let address = address();
        let formulas = FormulaSet::new([(
            address.clone(),
            literal(Distribution::normal(0.0, 1.0).unwrap()),
        )])
        .unwrap();
        let reference = Formula::Reference { address };
        let report = formulas
            .sample_joint(
                &project(),
                &[
                    reference.clone(),
                    reference.clone(),
                    Formula::Ratio {
                        numerator: Box::new(reference.clone()),
                        denominator: Box::new(reference),
                    },
                ],
                config(7),
            )
            .unwrap();
        assert_eq!(report.estimates[2].mean, Some(1.0));
        assert_eq!(report.estimates[2].variance, Some(0.0));
        assert_eq!(report.covariance[0][1], report.estimates[0].variance);
    }

    #[test]
    fn exact_moments_fall_within_error_derived_tolerances() {
        let distribution = Distribution::normal(3.0, 2.0).unwrap();
        let report = FormulaSet::default()
            .sample_joint(&project(), &[literal(distribution.clone())], config(99))
            .unwrap();
        let estimate = &report.estimates[0];
        assert!(
            (estimate.mean.unwrap() - distribution.mean()).abs()
                <= 5.0 * estimate.mean_standard_error.unwrap()
        );
        assert!(
            (estimate.variance.unwrap() - distribution.variance()).abs()
                <= 5.0 * estimate.variance_standard_error.unwrap()
        );
    }

    #[test]
    fn every_primitive_family_matches_exact_moments_with_reported_error() {
        let distributions = [
            Distribution::point(3.0).unwrap(),
            Distribution::normal(-2.0, 1.5).unwrap(),
            Distribution::log_normal(0.2, 0.3).unwrap(),
            Distribution::beta(2.0, 5.0).unwrap(),
            Distribution::scaled_beta(3.0, 2.0, -2.0, 4.0).unwrap(),
        ];
        for (index, distribution) in distributions.into_iter().enumerate() {
            let config = MonteCarloConfig::new(index as u64, 10_000, 10_000, 1e-12, 0.0).unwrap();
            let report = FormulaSet::default()
                .sample_joint(&project(), &[literal(distribution.clone())], config)
                .unwrap();
            let estimate = &report.estimates[0];
            let mean_error = estimate.mean_standard_error.unwrap();
            let variance_error = estimate.variance_standard_error.unwrap();
            let mean_tolerance = (5.0 * mean_error).max(1e-12);
            let variance_tolerance = (5.0 * variance_error).max(1e-12);
            assert!((estimate.mean.unwrap() - distribution.mean()).abs() <= mean_tolerance);
            assert!(
                (estimate.variance.unwrap() - distribution.variance()).abs() <= variance_tolerance
            );
        }
    }

    #[test]
    fn sums_obey_linearity_and_shared_variance_identity() {
        let shared_address = address();
        let formulas = FormulaSet::new([(
            shared_address.clone(),
            literal(Distribution::normal(3.0, 2.0).unwrap()),
        )])
        .unwrap();
        let shared = Formula::Reference {
            address: shared_address,
        };
        let doubled = Formula::Sum {
            terms: vec![shared.clone(), shared.clone()],
        };
        let report = formulas
            .sample_joint(&project(), &[shared, doubled], config(314))
            .unwrap();
        let single = &report.estimates[0];
        let double = &report.estimates[1];
        assert!((double.mean.unwrap() - 2.0 * single.mean.unwrap()).abs() < 1e-12);
        assert!((double.variance.unwrap() - 4.0 * single.variance.unwrap()).abs() < 1e-12);
        assert!((report.covariance[0][1].unwrap() - 2.0 * single.variance.unwrap()).abs() < 1e-12);
    }

    #[test]
    fn bounded_clamps_and_invalid_ratios_are_counted() {
        let bounded = Formula::Bounded {
            input: Box::new(literal(Distribution::point(10.0).unwrap())),
            lower: -1.0,
            upper: 1.0,
        };
        let bounded_report = FormulaSet::default()
            .sample_joint(&project(), &[bounded], config(1))
            .unwrap();
        assert_eq!(bounded_report.estimates[0].mean, Some(1.0));
        let ratio = Formula::Ratio {
            numerator: Box::new(literal(Distribution::point(1.0).unwrap())),
            denominator: Box::new(literal(Distribution::point(0.0).unwrap())),
        };
        let invalid_report = FormulaSet::default()
            .sample_joint(
                &project(),
                &[ratio],
                MonteCarloConfig::new(1, 2, 10, 0.1, 0.0).unwrap(),
            )
            .unwrap();
        assert_eq!(
            invalid_report.diagnostics.invalid_samples.zero_denominator,
            10
        );
        assert_eq!(
            invalid_report.diagnostics.status,
            ConvergenceStatus::InsufficientValidSamples
        );
        assert_eq!(invalid_report.estimates[0].mean, None);
        let overflow = Formula::Product {
            factors: vec![
                literal(Distribution::point(f64::MAX).unwrap()),
                literal(Distribution::point(f64::MAX).unwrap()),
            ],
        };
        let overflow_report = FormulaSet::default()
            .sample_joint(
                &project(),
                &[overflow],
                MonteCarloConfig::new(1, 2, 3, 0.1, 0.0).unwrap(),
            )
            .unwrap();
        assert_eq!(
            overflow_report
                .diagnostics
                .invalid_samples
                .non_finite_result,
            3
        );
    }

    #[test]
    fn project_cycle_and_unit_validation_run_before_sampling() {
        let local = address();
        let foreign = EstimateAddress::new(
            ProjectId::new("foreign").unwrap(),
            EstimateOwner::Node(EntityId::new(1)),
            EstimateId::new(1),
        );
        let cross_project = Formula::Reference { address: foreign };
        assert!(matches!(
            FormulaSet::default().sample_joint(&project(), &[cross_project], config(1)),
            Err(MonteCarloError::Formula(
                FormulaError::CrossProjectReference { .. }
            ))
        ));

        let cyclic = FormulaSet::new([(
            local.clone(),
            Formula::Reference {
                address: local.clone(),
            },
        )])
        .unwrap();
        assert!(matches!(
            cyclic.sample_joint(
                &project(),
                &[Formula::Reference { address: local }],
                config(1)
            ),
            Err(MonteCarloError::Formula(FormulaError::ReferenceCycle(_)))
        ));

        let mismatched = Formula::Sum {
            terms: vec![
                literal(Distribution::point(1.0).unwrap()),
                Formula::Literal {
                    distribution: Distribution::point(1.0).unwrap(),
                    unit: Unit::base("m").unwrap(),
                },
            ],
        };
        assert!(matches!(
            FormulaSet::default().sample_joint(&project(), &[mismatched], config(1)),
            Err(MonteCarloError::Formula(FormulaError::UnitMismatch { .. }))
        ));
    }

    #[test]
    fn dependence_jointly_samples_addressed_marginals() {
        let left = address();
        let right = EstimateAddress::new(
            project(),
            EstimateOwner::Node(EntityId::new(2)),
            EstimateId::new(1),
        );
        let formulas = FormulaSet::new([
            (
                left.clone(),
                literal(Distribution::normal(0.0, 1.0).unwrap()),
            ),
            (
                right.clone(),
                literal(Distribution::normal(0.0, 1.0).unwrap()),
            ),
        ])
        .unwrap();
        let dependence = ProjectDependenceModel {
            revision: 0,
            residual_groups: vec![ResidualDependenceGroup {
                members: vec![left.clone(), right.clone()],
                correlation: GaussianCopulaCorrelation {
                    scale: CorrelationScale::Latent,
                    matrix: vec![vec![1.0, 1.0], vec![1.0, 1.0]],
                },
            }],
        };
        let roots = [
            Formula::Reference { address: left },
            Formula::Reference { address: right },
        ];
        let first = formulas
            .sample_joint_with_dependence(&project(), &roots, config(88), &dependence)
            .unwrap();
        let second = formulas
            .sample_joint_with_dependence(&project(), &roots, config(88), &dependence)
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.estimates[0], first.estimates[1]);
        assert_eq!(first.covariance[0][1], first.estimates[0].variance);
    }

    #[test]
    fn dependence_requires_declared_literal_marginals() {
        let left = address();
        let right = EstimateAddress::new(
            project(),
            EstimateOwner::Node(EntityId::new(2)),
            EstimateId::new(1),
        );
        let group = ResidualDependenceGroup {
            members: vec![left.clone(), right.clone()],
            correlation: GaussianCopulaCorrelation {
                scale: CorrelationScale::Latent,
                matrix: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
            },
        };
        let model = ProjectDependenceModel {
            revision: 0,
            residual_groups: vec![group],
        };
        let formulas = FormulaSet::new([(
            left.clone(),
            Formula::Bounded {
                input: Box::new(literal(Distribution::normal(0.0, 1.0).unwrap())),
                lower: -1.0,
                upper: 1.0,
            },
        )])
        .unwrap();
        let roots = [Formula::Reference {
            address: left.clone(),
        }];
        assert!(matches!(
            formulas.sample_joint_with_dependence(&project(), &roots, config(1), &model),
            Err(MonteCarloError::NonMarginalDependenceMember(address)) if address == left
        ));
    }
}
