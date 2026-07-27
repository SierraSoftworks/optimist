//! Rendering system design reports as aligned tables.
//!
//! An uncertain quantity is shown as its mean with a central eighty percent
//! interval beside it. A single number would hide the spread that the whole
//! model exists to carry, and the full distribution does not fit in a column,
//! so the interval is the compromise that keeps a reader honest about how much
//! is known.

use crate::{
    squiggle::Value,
    system::{Bottleneck, Comparison, Evaluation, LoadedSystem},
};

use super::{output_table::rows, system};

pub(super) fn summary(loaded: &LoadedSystem) -> String {
    let scale_units = loaded.model.scale_units.len();
    rows(
        "PROPERTY\tVALUE",
        [
            format!("name\t{}", loaded.name),
            format!("components\t{}", loaded.model.components.len()),
            format!("relationships\t{}", loaded.model.relationships.len()),
            format!("shared quantities\t{}", loaded.model.scratchpad.len()),
            format!("scale units\t{scale_units}"),
            format!("interventions\t{}", loaded.model.interventions.len()),
            format!("component types\t{}", loaded.component_types.len()),
            format!("behaviours\t{}", loaded.mutators.len()),
        ]
        .into_iter(),
    )
}

pub(super) fn catalogue(loaded: &LoadedSystem) -> String {
    let types = loaded.component_types.values().map(|component| {
        format!(
            "component\t{}\t{}\t{}",
            component.id,
            component.properties.len(),
            component.constraints.len()
        )
    });
    let behaviours = loaded.mutators.values().map(|mutator| {
        format!(
            "behaviour\t{}\t{}\t{}",
            mutator.id,
            mutator.properties.len(),
            mutator.transforms.len()
        )
    });
    rows("KIND\tID\tPROPERTIES\tLIMITS", types.chain(behaviours))
}

pub(super) fn channels(evaluation: &Evaluation, only: Option<&str>) -> String {
    let step = evaluation.settled();
    let entries = system::channels(evaluation, only)
        .into_iter()
        .flat_map(|(id, channels)| {
            channels
                .iter()
                .map(move |(name, value)| format!("{id}\t{name}\t{}", quantity(value)))
        });
    let table = rows("COMPONENT\tCHANNEL\tVALUE", entries);
    if step.converged {
        return table;
    }
    // A model that did not settle has no steady state to report, and saying so
    // matters more than the numbers that happened to be reached.
    format!(
        "{table}\n\nDid not settle after {} passes; largest movement {:.3e}.\n\
         A loop whose gain exceeds one has no steady state to find.",
        step.iterations, step.movement
    )
}

pub(super) fn bottlenecks(ranked: &[Bottleneck]) -> String {
    rows(
        "COMPONENT\tCONSTRAINT\tUTILISATION\tP90\tBINDS\tREPLICAS\tHEADROOM",
        ranked.iter().map(|entry| {
            format!(
                "{}\t{}\t{:.3}\t{:.3}\t{}\t{:.0}\t{:.4}",
                entry.component,
                entry.constraint,
                entry.utilisation,
                entry.utilisation_p90,
                share(entry.probability_of_binding),
                entry.replicas,
                entry.headroom
            )
        }),
    )
}

pub(super) fn comparison(comparison: &Comparison) -> String {
    let movements = rows(
        "COMPONENT\tCONSTRAINT\tBEFORE\tAFTER\tBOUND BEFORE\tBOUND AFTER\tEFFECT",
        comparison.movements.iter().map(|movement| {
            let effect = if movement.relieved() {
                "relieved"
            } else if movement.introduced() {
                "introduced"
            } else if movement.shift() < 0.0 {
                "eased"
            } else if movement.shift() > 0.0 {
                "loaded"
            } else {
                "unchanged"
            };
            format!(
                "{}\t{}\t{:.3}\t{:.3}\t{}\t{}\t{effect}",
                movement.component,
                movement.constraint,
                movement.before,
                movement.after,
                share(movement.bound_before),
                share(movement.bound_after)
            )
        }),
    );
    let introduced = comparison.introduced().len();
    if introduced == 0 {
        return movements;
    }
    format!(
        "{movements}\n\n{introduced} constraint(s) started binding under this change.\n\
         Relieving one limit routinely promotes another, so check whether this is a fix or a move."
    )
}

/// Renders a quantity as its mean and central eighty percent interval.
fn quantity(value: &Value) -> String {
    match value {
        Value::Number(number) => format!("{number:.4}"),
        Value::Distribution(distribution) => {
            let (Ok(mean), Ok(low), Ok(high)) = (
                distribution.mean(),
                distribution.quantile(0.1),
                distribution.quantile(0.9),
            ) else {
                return "unavailable".to_owned();
            };
            format!("{mean:.4} [{low:.4} .. {high:.4}]")
        }
        other => other.type_name().to_owned(),
    }
}

fn share(probability: f64) -> String {
    format!("{:.0}%", probability * 100.0)
}

/// A quantity reduced to the summary a report can carry.
#[derive(serde::Serialize)]
pub(super) struct ChannelSummary {
    mean: f64,
    p10: f64,
    p90: f64,
    certain: bool,
}

/// Projects a solved step into a structure a machine can read.
pub(super) fn channel_values(
    evaluation: &Evaluation,
    only: Option<&str>,
) -> std::collections::BTreeMap<String, std::collections::BTreeMap<String, ChannelSummary>> {
    system::channels(evaluation, only)
        .into_iter()
        .map(|(id, channels)| {
            let summarised = channels
                .iter()
                .filter_map(|(name, value)| Some((name.clone(), summarise(value)?)))
                .collect();
            (id.to_string(), summarised)
        })
        .collect()
}

fn summarise(value: &Value) -> Option<ChannelSummary> {
    match value {
        Value::Number(number) => Some(ChannelSummary {
            mean: *number,
            p10: *number,
            p90: *number,
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
