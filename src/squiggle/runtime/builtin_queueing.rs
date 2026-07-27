//! Little's Law and stationary queueing results for capacity models.
//!
//! # Little's Law
//!
//! For any stable system observed over a long interval, the mean number in the
//! system equals the mean arrival rate multiplied by the mean time each arrival
//! spends there:
//!
//! $$L = \lambda W$$
//!
//! The law is distribution-free. It assumes only that the system is stable, that
//! arrivals and departures balance over the averaging interval, and that all
//! three quantities are measured over that same interval. It does not assume
//! Poisson arrivals, exponential service, a queueing discipline, or independence.
//! This is why it applies unchanged to connections held on a server, records
//! retained in a store, and messages resident in a queue.
//!
//! # Stationary queueing
//!
//! The waiting-time results are not distribution-free. They describe M/M/c:
//! Poisson arrivals, exponential service, `c` identical servers, one unbounded
//! first-come-first-served queue, and steady state. Real services violate all of
//! these to some degree. Exponential service is the important assumption,
//! because it maximises variability among distributions with a given mean, so a
//! service time that is more regular than exponential will queue less than these
//! results predict and one that is heavier-tailed will queue more.
//!
//! Offered load in erlangs is $a = \lambda / \mu = \lambda S$ for mean service
//! time $S$, and utilisation is $\rho = a / c$. Erlang B gives the blocking
//! probability of a loss system with no queue, computed by the numerically
//! stable recursion
//!
//! $$B(0, a) = 1, \qquad B(n, a) = \frac{a\,B(n-1, a)}{n + a\,B(n-1, a)}$$
//!
//! which avoids the overflow of evaluating factorials directly. Erlang C gives
//! the probability that an arrival must wait in a delay system,
//!
//! $$C(c, a) = \frac{B(c, a)}{1 - \rho\,(1 - B(c, a))}$$
//!
//! and the mean waiting time before service begins is
//!
//! $$W_q = \frac{C(c, a)\,S}{c\,(1 - \rho)}$$
//!
//! Residence time is $W_q + S$ and is deliberately not returned directly, so
//! that a model states whether it means queueing delay or total sojourn.
//!
//! # Saturation
//!
//! No stationary result exists at $\rho \geq 1$: the queue grows without bound
//! and the mean is infinite. Utilisation is therefore clamped just below one, so
//! a saturated queue yields a very large but finite delay. That value is a
//! saturation sentinel rather than a prediction, and the honest reading of it is
//! that demand has exceeded capacity, not that the delay will take that value.
//!
//! References: John D. C. Little, "A Proof for the Queuing Formula $L = \lambda
//! W$", *Operations Research* 9(3), 1961; Leonard Kleinrock, *Queueing Systems,
//! Volume 1: Theory* (1975), chapters 3 and 4; ITU-T Recommendation E.521 for
//! the Erlang recursions.

use crate::squiggle::{Diagnostic, Value, ast::Span};

use super::{
    Runtime,
    elementwise::{elementwise, finite},
};

/// Largest utilisation a stationary result is evaluated at.
///
/// Chosen so a saturated queue reports a delay around six orders of magnitude
/// above its service time: unmistakable against any real budget, while staying
/// far inside the range where the reciprocal is exact in double precision.
const MAX_UTILISATION: f64 = 1.0 - 1e-6;

builtins! {
    context(runtime, span);
        "Little.occupancy"(rate: (Number | Distribution), residence: (Number | Distribution)) =>
            elementwise(runtime, &[rate.clone(), residence.clone()], span, |row| {
                finite(row[0] * row[1], "occupancy")
            }),
        "Little.residence"(occupancy: (Number | Distribution), rate: (Number | Distribution)) =>
            elementwise(runtime, &[occupancy.clone(), rate.clone()], span, |row| {
                ratio(row[0], row[1], "residence time")
            }),
        "Little.rate"(occupancy: (Number | Distribution), residence: (Number | Distribution)) =>
            elementwise(runtime, &[occupancy.clone(), residence.clone()], span, |row| {
                ratio(row[0], row[1], "arrival rate")
            }),
        "Queue.utilisation" | "Queue.utilization"(demand: (Number | Distribution), capacity: (Number | Distribution)) =>
            elementwise(runtime, &[demand.clone(), capacity.clone()], span, |row| {
                ratio(row[0], row[1], "utilisation")
            }),
        "Queue.mm1Wait"(service: (Number | Distribution), utilisation: (Number | Distribution)) =>
            elementwise(runtime, &[service.clone(), utilisation.clone()], span, |row| {
                let rho = stable(row[1]);
                finite(rho * row[0] / (1.0 - rho), "queueing delay")
            }),
        "Queue.mmcWait"(service: (Number | Distribution), servers: (Number | Distribution), utilisation: (Number | Distribution)) =>
            elementwise(runtime, &[service.clone(), servers.clone(), utilisation.clone()], span, |row| {
                let servers = count(row[1])?;
                let rho = stable(row[2]);
                let delay = erlang_c(servers, rho * servers as f64) * row[0]
                    / (servers as f64 * (1.0 - rho));
                finite(delay, "queueing delay")
            }),
        "Queue.erlangB"(servers: (Number | Distribution), offered: (Number | Distribution)) =>
            elementwise(runtime, &[servers.clone(), offered.clone()], span, |row| {
                finite(erlang_b(count(row[0])?, load(row[1])?), "blocking probability")
            }),
        "Queue.erlangC"(servers: (Number | Distribution), offered: (Number | Distribution)) =>
            elementwise(runtime, &[servers.clone(), offered.clone()], span, |row| {
                finite(erlang_c(count(row[0])?, load(row[1])?), "waiting probability")
            }),
}

fn ratio(numerator: f64, denominator: f64, what: &str) -> Result<f64, String> {
    if denominator == 0.0 {
        return Err(format!("{what} is undefined when its divisor is zero"));
    }
    finite(numerator / denominator, what)
}

fn stable(utilisation: f64) -> f64 {
    utilisation.clamp(0.0, MAX_UTILISATION)
}

fn count(servers: f64) -> Result<usize, String> {
    if !servers.is_finite() || servers < 0.5 {
        return Err("a queue requires at least one server".to_owned());
    }
    Ok(servers.round() as usize)
}

fn load(offered: f64) -> Result<f64, String> {
    if !offered.is_finite() || offered < 0.0 {
        return Err("offered load must be finite and non-negative".to_owned());
    }
    Ok(offered)
}

fn erlang_b(servers: usize, offered: f64) -> f64 {
    (1..=servers).fold(1.0, |blocking, server| {
        let scaled = offered * blocking;
        scaled / (server as f64 + scaled)
    })
}

fn erlang_c(servers: usize, offered: f64) -> f64 {
    let blocking = erlang_b(servers, offered);
    let utilisation = stable(offered / servers as f64);
    let denominator = 1.0 - utilisation * (1.0 - blocking);
    if denominator <= 0.0 {
        return 1.0;
    }
    (blocking / denominator).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erlang_b_matches_the_closed_form_for_small_systems() {
        // B(1, a) = a / (1 + a); B(2, a) = a^2 / (2 + 2a + a^2).
        for offered in [0.25_f64, 1.0, 4.0] {
            assert!((erlang_b(1, offered) - offered / (1.0 + offered)).abs() < 1e-12);
            let expected = offered.powi(2) / (2.0 + 2.0 * offered + offered.powi(2));
            assert!((erlang_b(2, offered) - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn erlang_b_falls_as_servers_are_added() {
        let blocking = (1..12).map(|c| erlang_b(c, 5.0)).collect::<Vec<_>>();
        assert!(blocking.windows(2).all(|pair| pair[1] < pair[0]));
    }

    #[test]
    fn a_single_server_delay_system_waits_with_probability_rho() {
        // C(1, a) reduces to the utilisation of an M/M/1 queue.
        for utilisation in [0.1_f64, 0.5, 0.9] {
            assert!((erlang_c(1, utilisation) - utilisation).abs() < 1e-12);
        }
    }

    #[test]
    fn erlang_c_dominates_erlang_b() {
        // Allowing arrivals to wait can only raise the chance of finding all
        // servers busy relative to turning them away.
        for servers in 1..8 {
            let offered = 0.6 * servers as f64;
            assert!(erlang_c(servers, offered) >= erlang_b(servers, offered));
        }
    }

    #[test]
    fn probabilities_stay_within_their_support() {
        for servers in 1..16 {
            for offered in [0.0_f64, 0.5, 5.0, 50.0] {
                assert!((0.0..=1.0).contains(&erlang_b(servers, offered)));
                assert!((0.0..=1.0).contains(&erlang_c(servers, offered)));
            }
        }
    }

    #[test]
    fn saturation_stays_finite() {
        assert!(erlang_c(4, 100.0).is_finite());
        assert!(stable(f64::INFINITY) <= MAX_UTILISATION);
        assert_eq!(stable(-1.0), 0.0);
    }

    #[test]
    fn fractional_server_counts_round_to_whole_servers() {
        assert_eq!(count(3.4).expect("rounds"), 3);
        assert_eq!(count(3.6).expect("rounds"), 4);
        assert!(count(0.2).is_err());
    }
}
