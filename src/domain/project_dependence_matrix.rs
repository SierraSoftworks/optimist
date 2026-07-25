use nalgebra::{DMatrix, DVector, SymmetricEigen};
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rand_distr::{Distribution as _, StandardNormal};
use statrs::distribution::{ContinuousCDF, Normal};

use super::{CorrelationScale, DependenceError, GaussianCopulaCorrelation, GaussianCopulaDraw};

const MATRIX_TOLERANCE: f64 = 1e-10;

/// A validated correlation matrix's reusable latent square root $Q\sqrt{\Lambda}$.
pub(super) struct CopulaFactor {
    root: DMatrix<f64>,
}

impl CopulaFactor {
    /// Draws one correlated latent vector and its standard Normal uniforms.
    pub(super) fn draw(&self, rng: &mut ChaCha20Rng) -> GaussianCopulaDraw {
        let dimension = self.root.nrows();
        let independent = DVector::from_iterator(
            dimension,
            (0..dimension).map(|_| StandardNormal.sample(rng)),
        );
        let normal = Normal::standard();
        let latent_normals: Vec<_> = (&self.root * independent).iter().copied().collect();
        let uniforms = latent_normals
            .iter()
            .map(|value| normal.cdf(*value))
            .collect();
        GaussianCopulaDraw {
            latent_normals,
            uniforms,
        }
    }
}

impl GaussianCopulaCorrelation {
    /// Validates matrix shape, values, symmetry, unit diagonal, and PSD.
    pub fn validate(&self) -> Result<(), DependenceError> {
        validate_matrix(&self.matrix)?;
        if self.scale == CorrelationScale::Rank {
            validate_psd(&self.latent_matrix())?;
        }
        Ok(())
    }

    /// Draws correlated latent Normals and uniforms from a pinned ChaCha20 stream.
    ///
    /// The symmetric eigendecomposition $R=Q\Lambda Q^\top$ yields the square
    /// root $Q\sqrt{\Lambda}$. Eigenvalues accepted within the documented PSD
    /// tolerance are clamped to zero, supporting exact singular correlations.
    /// Equal inputs and seed are bit-reproducible for Optimist's pinned random,
    /// distribution, and linear-algebra crate versions; sequences are not stable
    /// across dependency upgrades.
    pub fn sample_seeded(&self, seed: u64) -> Result<GaussianCopulaDraw, DependenceError> {
        self.validate()?;
        let mut rng = ChaCha20Rng::seed_from_u64(seed);
        Ok(self.sample(&mut rng))
    }

    pub(super) fn sample(&self, rng: &mut ChaCha20Rng) -> GaussianCopulaDraw {
        self.factored().draw(rng)
    }

    /// Precomputes the latent square root so repeated draws avoid eigensolving.
    ///
    /// A copula coupling $n$ estimates is drawn once per Monte Carlo iteration,
    /// so decomposing $R$ per draw would dominate an analysis. The factor depends
    /// only on the matrix, making this pure caching rather than a change of model.
    pub(super) fn factored(&self) -> CopulaFactor {
        let matrix = self.latent_matrix();
        let dimension = matrix.len();
        let eigen = SymmetricEigen::new(DMatrix::from_fn(dimension, dimension, |row, column| {
            matrix[row][column]
        }));
        let scales = DMatrix::from_diagonal(&eigen.eigenvalues.map(|value| value.max(0.0).sqrt()));
        CopulaFactor {
            root: eigen.eigenvectors * scales,
        }
    }

    pub(super) fn latent_matrix(&self) -> Vec<Vec<f64>> {
        match self.scale {
            CorrelationScale::Latent => self.matrix.clone(),
            CorrelationScale::Rank => self
                .matrix
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|value| 2.0 * (std::f64::consts::PI * value / 6.0).sin())
                        .collect()
                })
                .collect(),
        }
    }
}

fn validate_matrix(matrix: &[Vec<f64>]) -> Result<(), DependenceError> {
    let dimension = matrix.len();
    if matrix.iter().any(|row| row.len() != dimension) {
        return Err(DependenceError::NotSquare);
    }
    for row in matrix {
        for value in row {
            if !value.is_finite() {
                return Err(DependenceError::NonFinite);
            }
            if !(-1.0..=1.0).contains(value) {
                return Err(DependenceError::OutOfRange);
            }
        }
    }
    for (row_index, row) in matrix.iter().enumerate() {
        if (row[row_index] - 1.0).abs() > MATRIX_TOLERANCE {
            return Err(DependenceError::InvalidDiagonal);
        }
        for (column_index, column) in matrix.iter().take(row_index).enumerate() {
            if (row[column_index] - column[row_index]).abs() > MATRIX_TOLERANCE {
                return Err(DependenceError::NotSymmetric);
            }
        }
    }
    validate_psd(matrix)
}

fn validate_psd(matrix: &[Vec<f64>]) -> Result<(), DependenceError> {
    let dimension = matrix.len();
    let eigenvalues = SymmetricEigen::new(DMatrix::from_fn(dimension, dimension, |row, column| {
        matrix[row][column]
    }))
    .eigenvalues;
    let scale = eigenvalues
        .iter()
        .fold(1.0_f64, |max, value| max.max(value.abs()));
    let tolerance = MATRIX_TOLERANCE * dimension as f64 * scale;
    if eigenvalues.iter().any(|value| *value < -tolerance) {
        Err(DependenceError::NotPositiveSemidefinite)
    } else {
        Ok(())
    }
}
