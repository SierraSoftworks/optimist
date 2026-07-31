//! Reassembling the shares of a divided solve.
//!
//! Each share solved the same model over its own draws, so the answer is not an
//! average of theirs — it is their draws laid end to end in share order. What was
//! one iteration over a thousand draws becomes several over a few hundred each,
//! and the values that come back are the same values.
//!
//! A share whose draws all agreed reports a plain number rather than a sample
//! set, which is why a share's width has to be carried alongside it: a number
//! standing for two hundred and fifty identical draws has to be laid out as two
//! hundred and fifty of them when its neighbours disagreed.

use std::collections::BTreeMap;

use crate::{squiggle::Value, system::values::from_draws};

use super::state::{ComponentState, Evaluation, LinkState, Step};

/// One share's result, and how many draws it carried.
pub(super) struct Share {
    pub(super) width: usize,
    pub(super) evaluation: Evaluation,
}

/// Lays every share's draws end to end into the answer for the whole ensemble.
pub(super) fn merge(shares: Vec<Share>) -> Evaluation {
    let Some(steps) = shares.first().map(|share| share.evaluation.steps.len()) else {
        return Evaluation { steps: Vec::new() };
    };
    let steps = (0..steps)
        .map(|index| {
            let at = shares
                .iter()
                .filter_map(|share| Some((share.width, share.evaluation.steps.get(index)?)))
                .collect::<Vec<_>>();
            step(&at)
        })
        .collect();
    Evaluation { steps }
}

fn step(shares: &[(usize, &Step)]) -> Step {
    let first = shares.first().map(|(_, step)| *step);
    Step {
        time: first.map_or(0.0, |step| step.time),
        components: keyed(shares, |step| &step.components, component),
        links: keyed(shares, |step| &step.links, link),
        // A divided solve has settled only where every share has, and the model
        // moved as far as the worst of them saw it move.
        converged: shares.iter().all(|(_, step)| step.converged),
        unsettled: shares
            .iter()
            .filter_map(|(_, step)| step.unsettled.clone())
            .max_by(|left, right| left.movement.total_cmp(&right.movement)),
        // A mixture found in any share is a mixture the design has; the share
        // that resolved the most states saw the most of it.
        mixture: shares
            .iter()
            .filter_map(|(_, step)| step.mixture.clone())
            .max_by_key(|mixture| mixture.states),
        iterations: shares
            .iter()
            .map(|(_, step)| step.iterations)
            .max()
            .unwrap_or_default(),
        movement: shares
            .iter()
            .map(|(_, step)| step.movement)
            .fold(0.0, f64::max),
    }
}

/// Merges one map that every share holds a version of.
fn keyed<Key: Clone + Ord, Held, Merged>(
    shares: &[(usize, &Step)],
    held: impl Fn(&Step) -> &BTreeMap<Key, Held>,
    merge: impl Fn(&[(usize, &Held)]) -> Merged,
) -> BTreeMap<Key, Merged> {
    let mut keys: Vec<Key> = shares
        .iter()
        .flat_map(|(_, step)| held(step).keys().cloned())
        .collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .map(|key| {
            let across = shares
                .iter()
                .filter_map(|(width, step)| Some((*width, held(step).get(&key)?)))
                .collect::<Vec<_>>();
            let merged = merge(&across);
            (key, merged)
        })
        .collect()
}

fn component(shares: &[(usize, &ComponentState)]) -> ComponentState {
    ComponentState {
        channels: quantities(shares, |state| &state.channels),
        requests: ported(shares, |state| &state.requests),
        responses: ported(shares, |state| &state.responses),
        arriving: ported(shares, |state| &state.arriving),
        returning: ported(shares, |state| &state.returning),
    }
}

fn link(shares: &[(usize, &LinkState)]) -> LinkState {
    LinkState {
        backlog: quantity(&picked(shares, |state| &state.backlog)),
        wait: quantity(&picked(shares, |state| &state.wait)),
        transit: quantity(&picked(shares, |state| &state.transit)),
        blocked: quantity(&picked(shares, |state| &state.blocked)),
        offered: quantity(&picked(shares, |state| &state.offered)),
        drain: quantity(&picked(shares, |state| &state.drain)),
        transfer: quantity(&picked(shares, |state| &state.transfer)),
        bandwidth: quantity(&picked(shares, |state| &state.bandwidth)),
    }
}

fn picked<'a, Held>(
    shares: &[(usize, &'a Held)],
    field: impl Fn(&'a Held) -> &'a Value,
) -> Vec<(usize, &'a Value)> {
    shares
        .iter()
        .map(|(width, held)| (*width, field(held)))
        .collect()
}

/// Merges a map of quantities that every share holds a version of.
fn quantities<'a, Held>(
    shares: &[(usize, &'a Held)],
    held: impl Fn(&'a Held) -> &'a BTreeMap<String, Value>,
) -> BTreeMap<String, Value> {
    across(shares, held, quantity)
}

/// Merges a map of per-port signal maps.
fn ported<'a, Held>(
    shares: &[(usize, &'a Held)],
    held: impl Fn(&'a Held) -> &'a BTreeMap<String, BTreeMap<String, Value>>,
) -> BTreeMap<String, BTreeMap<String, Value>> {
    across(shares, held, |signals| {
        across(signals, |signals| signals, quantity)
    })
}

fn across<'a, Held, Inner: 'a, Merged>(
    shares: &[(usize, &'a Held)],
    held: impl Fn(&'a Held) -> &'a BTreeMap<String, Inner>,
    merge: impl Fn(&[(usize, &'a Inner)]) -> Merged,
) -> BTreeMap<String, Merged> {
    let mut names: Vec<String> = shares
        .iter()
        .flat_map(|(_, share)| held(share).keys().cloned())
        .collect();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .map(|name| {
            let entries = shares
                .iter()
                .filter_map(|(width, share)| Some((*width, held(share).get(&name)?)))
                .collect::<Vec<_>>();
            let merged = merge(&entries);
            (name, merged)
        })
        .collect()
}

/// Lays one quantity's shares end to end.
///
/// A share that collapsed to a number stands for as many draws as it carried, so
/// it is spread back across them rather than contributing a single value.
fn quantity(shares: &[(usize, &Value)]) -> Value {
    if let [(_, only)] = shares {
        return (*only).clone();
    }
    let mut draws = Vec::new();
    for (width, value) in shares {
        match value {
            Value::Number(number) => draws.extend(std::iter::repeat_n(*number, *width)),
            Value::Distribution(distribution) => match distribution.samples() {
                Some(samples) => draws.extend_from_slice(samples),
                None => return (*value).clone(),
            },
            _ => return (*value).clone(),
        }
    }
    from_draws(draws).unwrap_or(Value::Number(0.0))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::{
        squiggle::distribution::Distribution,
        system::{
            evaluate::state::{Mixture, Unsettled},
            model::ComponentId,
        },
    };

    use super::*;

    fn spread(draws: &[f64]) -> Value {
        Value::Distribution(Distribution::from_samples(draws.to_vec()).expect("a spread"))
    }

    /// Reads a quantity back as the draws it stands for.
    fn drawn(value: &Value) -> Vec<f64> {
        match value {
            Value::Number(number) => vec![*number],
            Value::Distribution(distribution) => distribution.samples().expect("samples").to_vec(),
            other => panic!("not a quantity: {other:?}"),
        }
    }

    fn share(width: usize, step: Step) -> Share {
        Share {
            width,
            evaluation: Evaluation { steps: vec![step] },
        }
    }

    /// A step carrying one channel on one component, and nothing else.
    fn carrying(channel: Value) -> Step {
        Step {
            time: 0.0,
            components: [(
                ComponentId::new("api"),
                ComponentState {
                    channels: [("rate".to_owned(), channel)].into(),
                    ..ComponentState::default()
                },
            )]
            .into(),
            links: BTreeMap::new(),
            converged: true,
            unsettled: None,
            mixture: None,
            iterations: 1,
            movement: 0.0,
        }
    }

    fn reassembled(shares: Vec<Share>) -> Vec<f64> {
        let merged = merge(shares);
        drawn(&merged.settled().components[&ComponentId::new("api")].channels["rate"])
    }

    /// The answer is the shares' draws in share order, not an average of them.
    #[test]
    fn shares_are_laid_end_to_end_in_order() {
        assert_eq!(
            reassembled(vec![
                share(2, carrying(spread(&[1.0, 2.0]))),
                share(3, carrying(spread(&[3.0, 4.0, 5.0]))),
            ]),
            vec![1.0, 2.0, 3.0, 4.0, 5.0]
        );
    }

    /// A share whose draws agreed reports a number, and it stands for all of them.
    #[rstest]
    #[case(3, vec![7.0, 7.0, 7.0, 1.0, 2.0])]
    #[case(1, vec![7.0, 1.0, 2.0])]
    fn a_collapsed_share_is_spread_across_the_draws_it_carried(
        #[case] width: usize,
        #[case] expected: Vec<f64>,
    ) {
        assert_eq!(
            reassembled(vec![
                share(width, carrying(Value::Number(7.0))),
                share(2, carrying(spread(&[1.0, 2.0]))),
            ]),
            expected
        );
    }

    /// Undivided work must come back untouched, however wide its share was.
    #[test]
    fn one_share_is_passed_through_unchanged() {
        assert_eq!(
            reassembled(vec![share(4, carrying(Value::Number(7.0)))]),
            vec![7.0]
        );
    }

    /// A divided solve has settled only where every share has.
    #[rstest]
    #[case(true, true, true)]
    #[case(true, false, false)]
    #[case(false, false, false)]
    fn settling_is_agreed_by_every_share(
        #[case] left: bool,
        #[case] right: bool,
        #[case] expected: bool,
    ) {
        let step = |converged| Step {
            converged,
            ..carrying(Value::Number(1.0))
        };
        let merged = merge(vec![share(1, step(left)), share(1, step(right))]);
        assert_eq!(merged.settled().converged, expected);
    }

    /// The model moved as far as the worst share saw it move.
    #[test]
    fn the_worst_share_decides_what_is_still_moving() {
        let step = |movement: f64| Step {
            movement,
            converged: false,
            unsettled: Some(Unsettled {
                component: ComponentId::new("api"),
                channel: format!("moving by {movement}"),
                movement,
                stalled: false,
            }),
            ..carrying(Value::Number(1.0))
        };
        let merged = merge(vec![share(1, step(0.1)), share(1, step(0.5))]);
        let settled = merged.settled();

        assert_eq!(settled.movement, 0.5);
        assert_eq!(
            settled
                .unsettled
                .as_ref()
                .expect("something moving")
                .channel,
            "moving by 0.5"
        );
    }

    /// A mixture found in any share is a mixture the design has.
    #[test]
    fn the_share_resolving_the_most_states_describes_the_mixture() {
        let step = |states: Option<usize>| Step {
            mixture: states.map(|states| Mixture {
                component: ComponentId::new("api"),
                channel: "rate".to_owned(),
                states,
                swing: 1.0,
            }),
            ..carrying(Value::Number(1.0))
        };
        let merged = merge(vec![
            share(1, step(None)),
            share(1, step(Some(2))),
            share(1, step(Some(3))),
        ]);

        assert_eq!(
            merged.settled().mixture.as_ref().expect("a mixture").states,
            3
        );
    }

    /// Every step of the horizon is reassembled, not only the last.
    #[test]
    fn the_whole_horizon_is_reassembled() {
        let at = |time: f64, draw: f64| Step {
            time,
            ..carrying(Value::Number(draw))
        };
        let merged = merge(vec![
            Share {
                width: 1,
                evaluation: Evaluation {
                    steps: vec![at(0.0, 1.0), at(1.0, 2.0)],
                },
            },
            Share {
                width: 1,
                evaluation: Evaluation {
                    steps: vec![at(0.0, 3.0), at(1.0, 4.0)],
                },
            },
        ]);

        let times: Vec<f64> = merged.steps.iter().map(|step| step.time).collect();
        assert_eq!(times, vec![0.0, 1.0]);
        assert_eq!(
            drawn(&merged.steps[0].components[&ComponentId::new("api")].channels["rate"]),
            vec![1.0, 3.0]
        );
    }

    /// Nothing to reassemble is an empty answer rather than a panic.
    #[test]
    fn no_shares_reassemble_to_nothing() {
        assert!(merge(Vec::new()).steps.is_empty());
    }
}
