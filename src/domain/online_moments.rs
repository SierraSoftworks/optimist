//! Numerically stable one-pass joint sample moments.
//!
//! For draw $x_n$, count $n$, previous mean $m_{n-1}$, and
//! $\delta=x_n-m_{n-1}$, the mean update is $m_n=m_{n-1}+\delta/n$.
//! The implementation applies the Pébay/Welford recurrences for unnormalized
//! central sums $M_2$, $M_3$, and $M_4$, then reports unbiased sample variance
//! $s^2=M_2/(n-1)$. Joint cross-products update as
//! $C_{xy,n}=C_{xy,n-1}+\delta_x(y_n-m_{y,n})$, yielding sample covariance
//! $C_{xy}/(n-1)$. This avoids retaining draws and reduces cancellation compared
//! with subtracting raw sums. It does not prevent overflow for extreme finite
//! values, nor does it correct autocorrelation; sample draws are pseudorandom
//! independent stream positions. See Pébay, *Formulas for Robust, One-Pass
//! Parallel Computation of Covariances and Arbitrary-Order Statistical Moments*,
//! SAND2008-6212, equations 1.5-1.8.

#[derive(Clone, Debug)]
pub(super) struct OnlineJointMoments {
    count: u64,
    means: Vec<f64>,
    second: Vec<f64>,
    third: Vec<f64>,
    fourth: Vec<f64>,
    cross: Vec<Vec<f64>>,
}

impl OnlineJointMoments {
    pub(super) fn new(dimensions: usize) -> Self {
        Self {
            count: 0,
            means: vec![0.0; dimensions],
            second: vec![0.0; dimensions],
            third: vec![0.0; dimensions],
            fourth: vec![0.0; dimensions],
            cross: vec![vec![0.0; dimensions]; dimensions],
        }
    }

    pub(super) fn push(&mut self, values: &[f64]) {
        let previous_count = self.count as f64;
        self.count += 1;
        let count = self.count as f64;
        let deltas: Vec<_> = values
            .iter()
            .zip(&self.means)
            .map(|(value, mean)| value - mean)
            .collect();
        for (mean, delta) in self.means.iter_mut().zip(&deltas) {
            *mean += delta / count;
        }
        for (row, delta) in deltas.iter().enumerate() {
            for (column, value) in values.iter().enumerate() {
                self.cross[row][column] += delta * (value - self.means[column]);
            }
        }
        for (index, delta) in deltas.iter().copied().enumerate() {
            let normalized = delta / count;
            let term = delta * normalized * previous_count;
            let previous_second = self.second[index];
            let previous_third = self.third[index];
            self.fourth[index] += term * normalized.powi(2) * (count.powi(2) - 3.0 * count + 3.0)
                + 6.0 * normalized.powi(2) * previous_second
                - 4.0 * normalized * previous_third;
            self.third[index] +=
                term * normalized * (count - 2.0) - 3.0 * normalized * previous_second;
            self.second[index] += term;
        }
    }

    pub(super) const fn count(&self) -> u64 {
        self.count
    }

    pub(super) fn mean(&self, index: usize) -> Option<f64> {
        (self.count > 0).then_some(self.means[index])
    }

    pub(super) fn variance(&self, index: usize) -> Option<f64> {
        (self.count > 1).then(|| self.second[index] / (self.count - 1) as f64)
    }

    pub(super) fn mean_standard_error(&self, index: usize) -> Option<f64> {
        self.variance(index)
            .map(|variance| (variance / self.count as f64).sqrt())
    }

    pub(super) fn variance_standard_error(&self, index: usize) -> Option<f64> {
        if self.count < 4 {
            return None;
        }
        let count = self.count as f64;
        let variance = self.variance(index)?;
        let fourth_moment = self.fourth[index] / count;
        let estimate = (fourth_moment - (count - 3.0) / (count - 1.0) * variance.powi(2)) / count;
        Some(estimate.max(0.0).sqrt())
    }

    pub(super) fn covariance(&self, row: usize, column: usize) -> Option<f64> {
        (self.count > 1).then(|| self.cross[row][column] / (self.count - 1) as f64)
    }
}
