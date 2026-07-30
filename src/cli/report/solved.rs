//! The quantities flowing through a solved design.
//!
//! Channels appear under their own names, and the signals travelling on a port
//! under the dotted name a manifest would use to read them: `in.<port>.<signal>`
//! for what arrived, `out.<port>.<signal>` for what came back. A reader chasing
//! a component's latency can therefore see the dependency latency that caused
//! it in the same table, which is the whole point of carrying it back.

use std::collections::BTreeMap;

use crate::{
    cli::render::{Cell, Column, Report, Table, Tone, number, quantity},
    squiggle::Value,
    system::{ComponentId, Evaluation, LoadedSystem, Signal},
};

/// Reports every solved quantity, a section per component.
pub(crate) fn channels(
    loaded: &LoadedSystem,
    evaluation: &Evaluation,
    only: Option<&str>,
) -> Report {
    let units = Units::new();
    let mut report = Report::default();
    for (component, quantities) in solved(evaluation, only) {
        let mut table = Table::new([
            Column::left("channel"),
            Column::right("mean"),
            Column::right("80% interval"),
        ]);
        for (name, value) in quantities {
            let (mean, interval) = quantity(&value, units.of(loaded, &component, &name));
            let tone = if name.starts_with("in.") || name.starts_with("out.") {
                Tone::Muted
            } else {
                Tone::Name
            };
            table.push([
                Cell::toned(&name, tone),
                Cell::plain(mean),
                Cell::toned(interval, Tone::Muted),
            ]);
        }
        report.section(component.to_string(), table);
    }

    if report.is_empty() {
        report.note(
            Tone::Warn,
            "Nothing to show",
            match only {
                Some(component) => format!("No component of this design is called `{component}`."),
                None => "This design has no components.".to_owned(),
            },
        );
        return report;
    }

    let step = evaluation.settled();
    if !step.converged {
        // A model that did not settle has no steady state to report, and saying
        // so matters more than the numbers that happened to be reached.
        report.note(
            Tone::Bad,
            "These figures mean nothing",
            format!(
                "The design did not settle after {} passes; the largest movement on the last \
                 one was {}{}. A loop whose gain exceeds one has no steady state to find, so \
                 these are the numbers the solver stopped at rather than the numbers the \
                 design reaches.",
                step.iterations,
                number(step.movement),
                step.unsettled
                    .as_ref()
                    .map(|moving| format!(", on `{}` of `{}`", moving.channel, moving.component))
                    .unwrap_or_default(),
            ),
        );
    } else if let Some(mixture) = &step.mixture {
        // Several stable states is a finding rather than a fault, and the
        // figures above are the mixture of them. The warning is that a mean
        // taken across two branches describes neither.
        report.note(
            Tone::Warn,
            format!("This design has {} stable states", mixture.states),
            format!(
                "`{}` of `{}` settled on {} values rather than one: the draws divide between \
                 branches and stay there. Every figure above is a mixture of those states, so \
                 read the interval rather than the mean — no request sees the average of two \
                 branches.",
                mixture.channel, mixture.component, mixture.states
            ),
        );
    }
    report
}

/// The unit each solved quantity was declared with.
///
/// A port signal is named for the signal it carries rather than for a channel,
/// so it takes its unit from the signal vocabulary; anything else is one of the
/// component's own channels. A quantity neither of them knows about is left
/// unannotated, which is the reading that changes nothing about how it is shown.
struct Units {
    signals: BTreeMap<String, Signal>,
}

impl Units {
    fn new() -> Self {
        Self {
            signals: crate::system::signals(),
        }
    }

    fn of<'a>(&'a self, loaded: &'a LoadedSystem, component: &ComponentId, name: &str) -> &'a str {
        if let Some((_, signal)) = name.rsplit_once('.') {
            return self
                .signals
                .get(signal)
                .map_or("", |signal| signal.unit.as_str());
        }
        loaded
            .model
            .components
            .iter()
            .find(|entry| &entry.id == component)
            .and_then(|entry| loaded.component_types.get(entry.component_type.as_str()))
            .and_then(|definition| definition.channels.get(name))
            .map_or("", |channel| channel.unit.as_str())
    }
}

/// A quantity reduced to the summary a machine-readable report can carry.
#[derive(serde::Serialize)]
pub(crate) struct ChannelSummary {
    mean: f64,
    p10: f64,
    p90: f64,
    certain: bool,
}

/// Projects a solved step into a structure a machine can read.
pub(crate) fn channel_values(
    evaluation: &Evaluation,
    only: Option<&str>,
) -> BTreeMap<String, BTreeMap<String, ChannelSummary>> {
    solved(evaluation, only)
        .into_iter()
        .map(|(component, quantities)| {
            let summarised = quantities
                .iter()
                .filter_map(|(name, value)| Some((name.clone(), summarise(value)?)))
                .collect();
            (component.to_string(), summarised)
        })
        .collect()
}

/// Lists each component's own channels, then the traffic on its ports.
///
/// Sorting the two together would file `in.requests.rate` between `hold_time`
/// and `offered`, which reads as though the component derived it. Keeping them
/// apart is what makes the second group recognisable as what the wires carried
/// rather than as something the component worked out.
fn solved(evaluation: &Evaluation, only: Option<&str>) -> Vec<(ComponentId, Vec<(String, Value)>)> {
    evaluation
        .settled()
        .components
        .iter()
        .filter(|(id, _)| only.is_none_or(|only| id.as_str() == only))
        .map(|(id, state)| {
            let ports = state
                .arriving
                .iter()
                .map(|(port, signals)| (format!("in.{port}"), signals))
                .chain(
                    state
                        .returning
                        .iter()
                        .map(|(port, signals)| (format!("out.{port}"), signals)),
                )
                .flat_map(|(prefix, signals)| {
                    signals
                        .iter()
                        .map(move |(signal, value)| (format!("{prefix}.{signal}"), value.clone()))
                });
            let quantities = state
                .channels
                .iter()
                .map(|(name, value)| (name.clone(), value.clone()))
                .chain(ports)
                .collect();
            (id.clone(), quantities)
        })
        .collect()
}

fn summarise(value: &Value) -> Option<ChannelSummary> {
    match value {
        Value::Number(figure) => Some(ChannelSummary {
            mean: *figure,
            p10: *figure,
            p90: *figure,
            certain: true,
        }),
        Value::Distribution(distribution) => Some(ChannelSummary {
            mean: distribution.mean().ok()?,
            p10: distribution.quantile(0.1).ok()?,
            p90: distribution.quantile(0.9).ok()?,
            certain: false,
        }),
        _ => None,
    }
}
