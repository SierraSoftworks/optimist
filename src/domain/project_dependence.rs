use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{EstimateAddress, ProjectId};

/// Identifies the interpretation of a Gaussian copula correlation matrix.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrelationScale {
    /// Spearman rank correlations, converted to latent Normal correlations.
    Rank,
    /// Correlations of the latent standard Normal variables themselves.
    Latent,
}

/// A validated rank or latent correlation matrix for a Gaussian copula.
///
/// For rank input, Optimist uses the Gaussian-copula identity
/// $\rho_z=2\sin(\pi\rho_s/6)$ to obtain latent Normal correlation. Both the
/// supplied and transformed matrices must be positive semidefinite (PSD).
/// Eigenvalues are accepted when $\lambda_{min}\geq -\tau$, where
/// $\tau=10^{-10}n\max(1,\max_i|\lambda_i|)$. This relative tolerance admits
/// roundoff-sized negative values and singular PSD matrices, but does not repair
/// materially inconsistent inputs. See Nelsen, *An Introduction to Copulas*,
/// section 5.1, and Higham, *Accuracy and Stability of Numerical Algorithms*,
/// chapter 10.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GaussianCopulaCorrelation {
    /// Whether entries are rank or latent Normal correlations.
    pub scale: CorrelationScale,
    /// Square correlation matrix in the same order as group members.
    pub matrix: Vec<Vec<f64>>,
}

/// One deterministic Gaussian-copula draw in latent and uniform coordinates.
///
/// If $Z\sim N(0,R)$, each returned uniform is $U_i=\Phi(Z_i)$. A caller maps
/// it to marginal CDF $F_i$ with $X_i=F_i^{-1}(U_i)$. This preserves each
/// marginal distribution while introducing only the dependence represented by
/// the copula. The model assumes continuous marginals; point masses and discrete
/// distributions can create ties whose rank correlations differ from the input.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GaussianCopulaDraw {
    /// Correlated latent standard Normal values.
    pub latent_normals: Vec<f64>,
    /// Latent values transformed through the standard Normal CDF.
    pub uniforms: Vec<f64>,
}

/// One non-overlapping set of addressed marginal estimates coupled by a copula.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResidualDependenceGroup {
    /// Unique estimate addresses in matrix row and column order.
    pub members: Vec<EstimateAddress>,
    /// Rank or latent Gaussian copula correlation matrix.
    pub correlation: GaussianCopulaCorrelation,
}

impl ResidualDependenceGroup {
    /// Validates membership, project scope, and correlation dimensions.
    pub fn validate(&self) -> Result<(), DependenceError> {
        if self.members.len() < 2 {
            return Err(DependenceError::TooFewMembers);
        }
        let unique: BTreeSet<_> = self.members.iter().collect();
        if unique.len() != self.members.len() {
            return Err(DependenceError::DuplicateMember);
        }
        let project = &self.members[0].project;
        if self.members.iter().any(|member| &member.project != project) {
            return Err(DependenceError::MixedProjects);
        }
        if self.correlation.matrix.len() != self.members.len() {
            return Err(DependenceError::DimensionMismatch);
        }
        self.correlation.validate()
    }
}

/// Project-level residual dependence document stored outside the causal graph.
///
/// Revision is the document revision used by replacement/removal commands. Groups
/// cannot overlap: one marginal has one residual dependence specification, avoiding
/// ambiguous composition of copulas. Empty group lists represent explicit
/// independence. Project persistence additionally verifies that every address
/// resolves to an estimate in the selected project.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProjectDependenceModel {
    /// Revision incremented on each successful replacement.
    pub revision: u64,
    /// Non-overlapping Gaussian copula residual groups.
    pub residual_groups: Vec<ResidualDependenceGroup>,
}

impl ProjectDependenceModel {
    /// Validates every group, project scope, and cross-group uniqueness.
    pub fn validate_for_project(&self, project: &ProjectId) -> Result<(), DependenceError> {
        let mut members = BTreeSet::new();
        for group in &self.residual_groups {
            group.validate()?;
            for member in &group.members {
                if &member.project != project {
                    return Err(DependenceError::CrossProjectAddress(member.clone()));
                }
                if !members.insert(member.clone()) {
                    return Err(DependenceError::OverlappingAddress(member.clone()));
                }
            }
        }
        Ok(())
    }
}

/// Validation failures for project dependence documents and Gaussian copulas.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum DependenceError {
    /// A dependence group must couple at least two estimates.
    #[error("a dependence group requires at least two members")]
    TooFewMembers,
    /// A dependence group repeats an estimate address.
    #[error("dependence group members must be unique")]
    DuplicateMember,
    /// A dependence group contains addresses from different projects.
    #[error("dependence group members must belong to one project")]
    MixedProjects,
    /// Matrix dimensions do not match group membership.
    #[error("correlation matrix dimensions must match group membership")]
    DimensionMismatch,
    /// A matrix row length differs from the matrix dimension.
    #[error("correlation matrix must be square")]
    NotSquare,
    /// At least one matrix entry is NaN or infinite.
    #[error("correlation matrix entries must be finite")]
    NonFinite,
    /// The matrix differs from its transpose beyond numerical tolerance.
    #[error("correlation matrix must be symmetric")]
    NotSymmetric,
    /// A diagonal entry differs from one beyond numerical tolerance.
    #[error("correlation matrix diagonal entries must equal one")]
    InvalidDiagonal,
    /// A matrix entry is outside the correlation interval.
    #[error("correlation matrix entries must lie in [-1, 1]")]
    OutOfRange,
    /// The matrix has a materially negative eigenvalue.
    #[error("correlation matrix must be positive semidefinite")]
    NotPositiveSemidefinite,
    /// An address belongs to a project other than the selected project.
    #[error("dependence address {0} belongs to another project")]
    CrossProjectAddress(EstimateAddress),
    /// An address occurs in more than one residual group.
    #[error("dependence address {0} occurs in multiple groups")]
    OverlappingAddress(EstimateAddress),
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand_chacha::ChaCha20Rng;

    use super::*;
    use crate::domain::{EntityId, EstimateId, EstimateOwner};

    fn address(project: &str, id: u64) -> EstimateAddress {
        EstimateAddress::new(
            ProjectId::new(project).unwrap(),
            EstimateOwner::Node(EntityId::new(id)),
            EstimateId::new(0),
        )
    }

    #[test]
    fn accepts_rank_and_singular_latent_matrices() {
        let rank = GaussianCopulaCorrelation {
            scale: CorrelationScale::Rank,
            matrix: vec![vec![1.0, 0.5], vec![0.5, 1.0]],
        };
        assert_eq!(rank.validate(), Ok(()));
        let singular = GaussianCopulaCorrelation {
            scale: CorrelationScale::Latent,
            matrix: vec![vec![1.0, 1.0], vec![1.0, 1.0]],
        };
        assert_eq!(singular.validate(), Ok(()));
        let draw = singular.sample_seeded(42).unwrap();
        assert_eq!(
            draw.latent_normals[0].to_bits(),
            draw.latent_normals[1].to_bits()
        );
        assert_eq!(draw, singular.sample_seeded(42).unwrap());
    }

    #[test]
    fn rejects_matrix_and_membership_violations() {
        for matrix in [
            vec![vec![1.0, 0.2], vec![0.3, 1.0]],
            vec![vec![1.0, 1.1], vec![1.1, 1.0]],
            vec![
                vec![1.0, 0.9, 0.9],
                vec![0.9, 1.0, -0.9],
                vec![0.9, -0.9, 1.0],
            ],
        ] {
            assert!(
                GaussianCopulaCorrelation {
                    scale: CorrelationScale::Latent,
                    matrix
                }
                .validate()
                .is_err()
            );
        }
        let duplicate = ResidualDependenceGroup {
            members: vec![address("p", 1), address("p", 1)],
            correlation: GaussianCopulaCorrelation {
                scale: CorrelationScale::Latent,
                matrix: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
            },
        };
        assert_eq!(duplicate.validate(), Err(DependenceError::DuplicateMember));
    }

    #[test]
    fn rejects_cross_project_and_overlapping_groups() {
        let correlation = GaussianCopulaCorrelation {
            scale: CorrelationScale::Latent,
            matrix: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        };
        let group = ResidualDependenceGroup {
            members: vec![address("p", 1), address("p", 2)],
            correlation: correlation.clone(),
        };
        let overlap = ResidualDependenceGroup {
            members: vec![address("p", 2), address("p", 3)],
            correlation,
        };
        let model = ProjectDependenceModel {
            revision: 0,
            residual_groups: vec![group, overlap],
        };
        assert!(matches!(
            model.validate_for_project(&ProjectId::new("p").unwrap()),
            Err(DependenceError::OverlappingAddress(_))
        ));
        assert!(matches!(
            model.validate_for_project(&ProjectId::new("q").unwrap()),
            Err(DependenceError::CrossProjectAddress(_))
        ));
    }

    #[test]
    fn empirical_rank_correlation_matches_sampling_error() {
        let target = 0.6_f64;
        let correlation = GaussianCopulaCorrelation {
            scale: CorrelationScale::Rank,
            matrix: vec![vec![1.0, target], vec![target, 1.0]],
        };
        let mut rng = ChaCha20Rng::seed_from_u64(99);
        let samples = 100_000_u64;
        let mut sums = [0.0; 5];
        for _ in 0..samples {
            let draw = correlation.sample(&mut rng);
            let [left, right] = draw.uniforms[..] else {
                unreachable!()
            };
            sums[0] += left;
            sums[1] += right;
            sums[2] += left * left;
            sums[3] += right * right;
            sums[4] += left * right;
        }
        let count = samples as f64;
        let covariance = sums[4] / count - sums[0] * sums[1] / count.powi(2);
        let left_variance = sums[2] / count - (sums[0] / count).powi(2);
        let right_variance = sums[3] / count - (sums[1] / count).powi(2);
        let observed = covariance / (left_variance * right_variance).sqrt();
        let standard_error = (1.0 - target.powi(2)) / (count - 3.0).sqrt();
        assert!((observed - target).abs() <= 5.0 * standard_error);
    }
}
