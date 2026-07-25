use rand_chacha::ChaCha20Rng;

use super::{Distribution, EffectProfile, EffectRelease, ScenarioAnalysisError};

/// One release form with its shape parameter resolved for a single draw.
pub(super) enum SampledRelease {
    Immediate,
    Linear { over: u64 },
    Exponential { half_life: f64 },
}

pub(super) struct SampledAftereffect {
    hold: Option<u64>,
    release: SampledRelease,
}

/// One temporal profile with every duration resolved to whole planning periods.
pub(super) struct SampledEffectProfile {
    ramp: u64,
    hold: Option<u64>,
    release: SampledRelease,
    aftereffect: Option<SampledAftereffect>,
}

impl SampledRelease {
    /// Fraction of full strength remaining `elapsed` periods into the release.
    ///
    /// With $k\geq0$ counting periods since the release began, the kernels are
    /// $\sigma_{\text{immediate}}(k)=0$,
    /// $\sigma_{\text{linear}}(k)=\max\left(0,\;1-\frac{k+1}{L+1}\right)$ over $L$ periods, and
    /// $\sigma_{\text{exp}}(k)=2^{-(k+1)/H}$ for a positive half-life $H$.
    ///
    /// Each kernel is offset by one period so that the first released period is
    /// already reduced, which makes $L=0$ agree exactly with the immediate form.
    /// The linear kernel reaches zero at $k=L$; the exponential kernel approaches
    /// zero asymptotically and is defined as zero for non-positive half-lives,
    /// where halving has no meaning.
    fn remaining(&self, elapsed: u64) -> f64 {
        let periods = elapsed.saturating_add(1) as f64;
        match self {
            Self::Immediate => 0.0,
            Self::Linear { over } => (1.0 - periods / over.saturating_add(1) as f64).max(0.0),
            Self::Exponential { half_life } if *half_life > 0.0 => {
                2.0_f64.powf(-periods / half_life)
            }
            Self::Exponential { .. } => 0.0,
        }
    }
}

impl SampledEffectProfile {
    /// Fraction of the primary effect active `elapsed` periods after arrival.
    ///
    /// For ramp width $r$ and hold width $h$,
    /// $$a(e)=\begin{cases}\frac{e+1}{r+1}&e<r\\ 1&r\leq e<r+h\\ \sigma(e-r-h)&e\geq r+h.\end{cases}$$
    ///
    /// An absent hold removes the third case entirely, leaving the monotone step
    /// that a permanent intervention applies. The ramp is likewise offset by one
    /// period so $r=0$ yields $a(0)=1$, making the default profile identical to an
    /// unshaped effect. The result always lies in $[0,1]$.
    pub(super) fn activation(&self, elapsed: u64) -> f64 {
        if elapsed < self.ramp {
            return elapsed.saturating_add(1) as f64 / self.ramp.saturating_add(1) as f64;
        }
        let Some(hold) = self.hold else {
            return 1.0;
        };
        match elapsed.checked_sub(self.ramp.saturating_add(hold)) {
            Some(released) => self.release.remaining(released),
            None => 1.0,
        }
    }

    /// Fraction of the rebound active `elapsed` periods after arrival.
    ///
    /// The rebound is anchored to the moment the primary effect starts releasing.
    /// With $k=e-r-h$ and rebound hold $h_a$,
    /// $$b(e)=\begin{cases}0&k<0\\ 1&0\leq k<h_a\\ \sigma_a(k-h_a)&k\geq h_a,\end{cases}$$
    /// and $b(e)=1$ for every $k\geq0$ when the rebound declares no hold window.
    /// Profiles without an aftereffect, and permanent profiles that never release,
    /// return zero for every period.
    pub(super) fn rebound(&self, elapsed: u64) -> f64 {
        let (Some(aftereffect), Some(hold)) = (self.aftereffect.as_ref(), self.hold) else {
            return 0.0;
        };
        let Some(released) = elapsed.checked_sub(self.ramp.saturating_add(hold)) else {
            return 0.0;
        };
        let Some(rebound_hold) = aftereffect.hold else {
            return 1.0;
        };
        match released.checked_sub(rebound_hold) {
            Some(faded) => aftereffect.release.remaining(faded),
            None => 1.0,
        }
    }
}

/// Resolves every uncertain duration in `profile` for one Monte Carlo draw.
///
/// A persistent profile consumes no randomness, so projects that shape no effects
/// observe exactly the sampling stream they would without profiles at all.
pub(super) fn sample(
    profile: &EffectProfile,
    rng: &mut ChaCha20Rng,
) -> Result<SampledEffectProfile, ScenarioAnalysisError> {
    Ok(SampledEffectProfile {
        ramp: optional_delay(profile.ramp.as_ref(), rng)?.unwrap_or(0),
        hold: optional_delay(profile.hold.as_ref(), rng)?,
        release: release(&profile.release, rng)?,
        aftereffect: profile
            .aftereffect
            .as_ref()
            .map(|value| {
                Ok(SampledAftereffect {
                    hold: optional_delay(value.hold.as_ref(), rng)?,
                    release: release(&value.release, rng)?,
                })
            })
            .transpose()?,
    })
}

fn release(
    value: &EffectRelease,
    rng: &mut ChaCha20Rng,
) -> Result<SampledRelease, ScenarioAnalysisError> {
    Ok(match value {
        EffectRelease::Immediate => SampledRelease::Immediate,
        EffectRelease::Linear { over } => SampledRelease::Linear {
            over: delay(&over.distribution, rng)?,
        },
        EffectRelease::Exponential { half_life } => SampledRelease::Exponential {
            half_life: finite(&half_life.distribution, rng)?,
        },
    })
}

fn optional_delay(
    estimate: Option<&super::Estimate<super::Duration>>,
    rng: &mut ChaCha20Rng,
) -> Result<Option<u64>, ScenarioAnalysisError> {
    estimate
        .map(|estimate| delay(&estimate.distribution, rng))
        .transpose()
}

/// Samples a non-negative duration and rounds it up to whole planning periods.
pub(super) fn delay(
    distribution: &Distribution,
    rng: &mut ChaCha20Rng,
) -> Result<u64, ScenarioAnalysisError> {
    periods(finite(distribution, rng)?)
}

/// Rounds an already-sampled duration up to whole planning periods.
///
/// Coupled durations are drawn through their copula rather than from the local
/// stream, so rounding is kept separate from sampling and both paths round a
/// value the same way once it exists.
pub(super) fn periods(value: f64) -> Result<u64, ScenarioAnalysisError> {
    if !value.is_finite() {
        return Err(ScenarioAnalysisError::NonFinitePrimitive);
    }
    Ok(value.ceil().min(u64::MAX as f64) as u64)
}

fn finite(
    distribution: &Distribution,
    rng: &mut ChaCha20Rng,
) -> Result<f64, ScenarioAnalysisError> {
    let value = distribution.sample(rng);
    if !value.is_finite() {
        return Err(ScenarioAnalysisError::NonFinitePrimitive);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use rand::{Rng, SeedableRng};

    use super::*;
    use crate::domain::{
        Duration, EffectAftereffect, Estimate, EstimateId, SquiggleEstimateDefinition, Unit,
    };

    fn periods(source: &str) -> Estimate<Duration> {
        let unit = Unit::base("duration").unwrap();
        Estimate::<Duration>::from_squiggle(
            EstimateId::new(0),
            SquiggleEstimateDefinition {
                source: source.to_owned(),
                seed: 42,
                sample_count: 256,
                target_unit: unit.clone(),
            },
            &unit,
        )
        .unwrap()
    }

    fn sampled(profile: &EffectProfile) -> SampledEffectProfile {
        sample(profile, &mut ChaCha20Rng::seed_from_u64(7)).unwrap()
    }

    #[test]
    fn persistent_profile_is_a_monotone_step_and_never_rebounds() {
        let profile = sampled(&EffectProfile::default());
        for elapsed in 0..16 {
            assert_eq!(profile.activation(elapsed), 1.0);
            assert_eq!(profile.rebound(elapsed), 0.0);
        }
    }

    #[test]
    fn persistent_profile_consumes_no_randomness() {
        let mut untouched = ChaCha20Rng::seed_from_u64(11);
        let mut sampled_first = ChaCha20Rng::seed_from_u64(11);
        sample(&EffectProfile::default(), &mut sampled_first).unwrap();
        assert_eq!(
            untouched.r#gen::<f64>(),
            sampled_first.r#gen::<f64>(),
            "profile sampling must not advance the stream for unshaped effects"
        );
    }

    #[test]
    fn bounded_hold_produces_a_rectangular_pulse() {
        let profile = sampled(
            &EffectProfile::new(
                None,
                Some(periods("pointMass(2)")),
                EffectRelease::Immediate,
                None,
            )
            .unwrap(),
        );
        assert_eq!(profile.activation(0), 1.0);
        assert_eq!(profile.activation(1), 1.0);
        assert_eq!(profile.activation(2), 0.0);
        assert_eq!(profile.activation(50), 0.0);
    }

    #[test]
    fn ramp_rises_linearly_to_full_strength() {
        let profile = sampled(
            &EffectProfile::new(
                Some(periods("pointMass(2)")),
                None,
                EffectRelease::Immediate,
                None,
            )
            .unwrap(),
        );
        assert!((profile.activation(0) - 1.0 / 3.0).abs() < 1e-12);
        assert!((profile.activation(1) - 2.0 / 3.0).abs() < 1e-12);
        assert_eq!(profile.activation(2), 1.0);
        assert_eq!(profile.activation(9), 1.0);
    }

    #[test]
    fn linear_release_reaches_zero_after_its_declared_span() {
        let profile = sampled(
            &EffectProfile::new(
                None,
                Some(periods("pointMass(1)")),
                EffectRelease::Linear {
                    over: periods("pointMass(3)"),
                },
                None,
            )
            .unwrap(),
        );
        assert_eq!(profile.activation(0), 1.0);
        assert!((profile.activation(1) - 0.75).abs() < 1e-12);
        assert!((profile.activation(2) - 0.50).abs() < 1e-12);
        assert!((profile.activation(3) - 0.25).abs() < 1e-12);
        assert_eq!(profile.activation(4), 0.0);
    }

    #[test]
    fn exponential_release_halves_over_its_half_life() {
        let profile = sampled(
            &EffectProfile::new(
                None,
                Some(periods("pointMass(1)")),
                EffectRelease::Exponential {
                    half_life: periods("pointMass(2)"),
                },
                None,
            )
            .unwrap(),
        );
        let first = profile.activation(1);
        assert!((first - 2.0_f64.powf(-0.5)).abs() < 1e-12);
        assert!((profile.activation(3) - first / 2.0).abs() < 1e-12);
    }

    #[test]
    fn rebound_fires_only_after_the_primary_effect_releases() {
        let profile = sampled(
            &EffectProfile::new(
                None,
                Some(periods("pointMass(2)")),
                EffectRelease::Immediate,
                Some(EffectAftereffect {
                    hold: Some(periods("pointMass(1)")),
                    release: EffectRelease::Immediate,
                }),
            )
            .unwrap(),
        );
        assert_eq!(profile.rebound(0), 0.0);
        assert_eq!(profile.rebound(1), 0.0);
        assert_eq!(profile.rebound(2), 1.0);
        assert_eq!(profile.rebound(3), 0.0);
    }

    #[test]
    fn activation_and_rebound_remain_within_the_unit_interval() {
        let profile = sampled(
            &EffectProfile::new(
                Some(periods("pointMass(3)")),
                Some(periods("pointMass(4)")),
                EffectRelease::Exponential {
                    half_life: periods("pointMass(2)"),
                },
                Some(EffectAftereffect {
                    hold: Some(periods("pointMass(2)")),
                    release: EffectRelease::Linear {
                        over: periods("pointMass(5)"),
                    },
                }),
            )
            .unwrap(),
        );
        for elapsed in 0..64 {
            let activation = profile.activation(elapsed);
            let rebound = profile.rebound(elapsed);
            assert!((0.0..=1.0).contains(&activation), "activation {activation}");
            assert!((0.0..=1.0).contains(&rebound), "rebound {rebound}");
        }
    }
}
