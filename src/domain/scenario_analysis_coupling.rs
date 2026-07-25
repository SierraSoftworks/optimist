use std::collections::BTreeMap;

use rand_chacha::ChaCha20Rng;

use super::{
    Distribution, EstimateId, EstimateOwner, ProjectDependenceModel, ProjectId,
    project_dependence_matrix::CopulaFactor,
};

/// Which copula and which position within it drives one sampled primitive.
#[derive(Clone, Copy)]
struct CouplingSlot {
    group: usize,
    member: usize,
}

/// Project residual dependence resolved against one analysis's primitives.
///
/// Coupling is addressed rather than structural: a group names estimate
/// addresses, so two primitives correlate because a modeller said they share a
/// residual cause, not because the graph connects them. Addresses outside this
/// project, or naming estimates this scenario never samples, simply never
/// resolve — their group is still drawn, which keeps every group's marginal
/// positions aligned with its matrix regardless of how much of it is in scope.
#[derive(Default)]
pub(super) struct Coupling {
    groups: Vec<CopulaFactor>,
    slots: BTreeMap<(EstimateOwner, EstimateId), CouplingSlot>,
}

impl Coupling {
    /// Indexes every coupled estimate belonging to `project`.
    pub(super) fn new(project: &ProjectId, model: Option<&ProjectDependenceModel>) -> Self {
        let mut coupling = Self::default();
        let Some(model) = model else {
            return coupling;
        };
        for (group, residual) in model.residual_groups.iter().enumerate() {
            for (member, address) in residual.members.iter().enumerate() {
                if &address.project == project {
                    coupling.slots.insert(
                        (address.owner.clone(), address.estimate),
                        CouplingSlot { group, member },
                    );
                }
            }
            coupling.groups.push(residual.correlation.factored());
        }
        coupling
    }

    /// Binds one estimate's distribution to its copula position, if it has one.
    pub(super) fn primitive(
        &self,
        owner: &EstimateOwner,
        estimate: EstimateId,
        distribution: &Distribution,
    ) -> CoupledPrimitive {
        CoupledPrimitive {
            distribution: distribution.clone(),
            slot: self.slots.get(&(owner.clone(), estimate)).copied(),
        }
    }

    /// Draws every group's uniforms for one Monte Carlo iteration.
    ///
    /// Groups are drawn together, before any independent primitive, so the
    /// random stream stays a deterministic function of the dependence document
    /// rather than of the order primitives happen to be visited. A project with
    /// no groups consumes no randomness here and reproduces its earlier results
    /// exactly.
    pub(super) fn draw(&self, rng: &mut ChaCha20Rng) -> CouplingDraw {
        CouplingDraw(
            self.groups
                .iter()
                .map(|group| group.draw(rng).uniforms)
                .collect(),
        )
    }
}

/// One iteration's copula uniforms, indexed by group and member.
pub(super) struct CouplingDraw(Vec<Vec<f64>>);

/// A sampled primitive that may be driven by a shared copula.
#[derive(Clone)]
pub(super) struct CoupledPrimitive {
    distribution: Distribution,
    slot: Option<CouplingSlot>,
}

impl CoupledPrimitive {
    /// Samples the primitive, preserving its marginal distribution either way.
    ///
    /// A coupled primitive takes its value from the copula uniform by inverse
    /// transform, $x=F^{-1}(u)$, which reproduces the authored marginal exactly
    /// while carrying the group's dependence. It therefore consumes no
    /// randomness of its own: its uncertainty was already drawn with its group.
    pub(super) fn sample(&self, rng: &mut ChaCha20Rng, draw: &CouplingDraw) -> f64 {
        match self.slot {
            Some(slot) => self
                .distribution
                .inverse_cdf(draw.0[slot.group][slot.member]),
            None => self.distribution.sample(rng),
        }
    }

    /// Returns the mean of the underlying marginal, which coupling never changes.
    pub(super) fn marginal_mean(&self) -> f64 {
        self.distribution.mean()
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;
    use crate::domain::{
        CorrelationScale, EntityId, EstimateAddress, GaussianCopulaCorrelation,
        ResidualDependenceGroup,
    };

    fn project() -> ProjectId {
        ProjectId::new("coupled").unwrap()
    }

    fn address(entity: u64) -> EstimateAddress {
        EstimateAddress::new(
            project(),
            EstimateOwner::Node(EntityId::new(entity)),
            EstimateId::new(0),
        )
    }

    fn model(correlation: f64) -> ProjectDependenceModel {
        ProjectDependenceModel {
            revision: 0,
            residual_groups: vec![ResidualDependenceGroup {
                members: vec![address(0), address(1)],
                correlation: GaussianCopulaCorrelation {
                    scale: CorrelationScale::Latent,
                    matrix: vec![vec![1.0, correlation], vec![correlation, 1.0]],
                },
            }],
        }
    }

    fn pair(correlation: f64, draws: usize) -> Vec<(f64, f64)> {
        let dependence = model(correlation);
        let coupling = Coupling::new(&project(), Some(&dependence));
        let normal = Distribution::normal(0.0, 1.0).unwrap();
        let left = coupling.primitive(
            &EstimateOwner::Node(EntityId::new(0)),
            EstimateId::new(0),
            &normal,
        );
        let right = coupling.primitive(
            &EstimateOwner::Node(EntityId::new(1)),
            EstimateId::new(0),
            &normal,
        );
        let mut rng = ChaCha20Rng::seed_from_u64(11);
        (0..draws)
            .map(|_| {
                let draw = coupling.draw(&mut rng);
                (left.sample(&mut rng, &draw), right.sample(&mut rng, &draw))
            })
            .collect()
    }

    #[test]
    fn perfect_correlation_makes_two_estimates_one_variable() {
        for (left, right) in pair(1.0, 64) {
            assert!(
                (left - right).abs() < 1e-9,
                "a unit correlation must reproduce one shared draw"
            );
        }
    }

    #[test]
    fn coupled_samples_recover_the_requested_correlation() {
        let samples = pair(0.8, 4_000);
        let count = samples.len() as f64;
        let mean = |values: &[f64]| values.iter().sum::<f64>() / count;
        let left: Vec<_> = samples.iter().map(|(value, _)| *value).collect();
        let right: Vec<_> = samples.iter().map(|(_, value)| *value).collect();
        let (left_mean, right_mean) = (mean(&left), mean(&right));
        let covariance = left
            .iter()
            .zip(&right)
            .map(|(a, b)| (a - left_mean) * (b - right_mean))
            .sum::<f64>()
            / count;
        let deviation = |values: &[f64], mean: f64| {
            (values
                .iter()
                .map(|value| (value - mean).powi(2))
                .sum::<f64>()
                / count)
                .sqrt()
        };
        let correlation =
            covariance / (deviation(&left, left_mean) * deviation(&right, right_mean));
        // Standard error of a Pearson correlation near 0.8 over 4,000 draws is
        // (1 - r^2)/sqrt(n) ~= 0.006, so four standard errors is a safe bound.
        assert!(
            (correlation - 0.8).abs() < 0.025,
            "sampled correlation {correlation} must match the copula"
        );
    }

    #[test]
    fn uncoupled_estimates_ignore_the_copula_and_stay_independent() {
        let dependence = model(1.0);
        let coupling = Coupling::new(&project(), Some(&dependence));
        let normal = Distribution::normal(0.0, 1.0).unwrap();
        let outside = coupling.primitive(
            &EstimateOwner::Node(EntityId::new(7)),
            EstimateId::new(0),
            &normal,
        );
        let mut rng = ChaCha20Rng::seed_from_u64(3);
        let draw = coupling.draw(&mut rng);
        let first = outside.sample(&mut rng, &draw);
        let second = outside.sample(&mut rng, &draw);
        assert_ne!(first, second);
    }

    #[test]
    fn an_absent_model_consumes_no_randomness() {
        let coupling = Coupling::new(&project(), None);
        let mut coupled = ChaCha20Rng::seed_from_u64(5);
        let mut plain = ChaCha20Rng::seed_from_u64(5);
        let normal = Distribution::normal(0.0, 1.0).unwrap();
        let primitive = coupling.primitive(
            &EstimateOwner::Node(EntityId::new(0)),
            EstimateId::new(0),
            &normal,
        );
        let draw = coupling.draw(&mut coupled);
        assert_eq!(
            primitive.sample(&mut coupled, &draw),
            normal.sample(&mut plain)
        );
    }
}
