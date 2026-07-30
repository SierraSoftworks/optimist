//! Choosing how a report is rendered.
//!
//! Two audiences read this tool: a person deciding what to build, and a script
//! or agent checking whether a change made things better. The first is served
//! by [`OutputFormat::Table`], which lays a report out for a terminal; the
//! second by JSON, which carries the same figures without the presentation.
//! Neither is a translation of the other — each command decides what its
//! machine-readable answer contains — but every figure a person can read is one
//! a script can reach too.

use clap::ValueEnum;

use crate::system::{Bottleneck, Comparison, Evaluation, LoadedSystem};

use super::{
    diagnose::{Diagnosis, Finding},
    output_json, report,
};

/// How a report should be written.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum OutputFormat {
    /// Boxed, coloured sections, for reading.
    Table,
    /// One JSON document, for a script that wants the whole answer.
    Json,
    /// One JSON document per line, for a script that streams.
    Jsonl,
}

/// When colour should be used.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub(super) enum ColourChoice {
    /// Colour a terminal, and nothing else.
    #[default]
    Auto,
    /// Colour whatever the output is going to.
    Always,
    /// Never colour.
    Never,
}

impl ColourChoice {
    /// Applies this choice to every report written afterwards.
    pub(super) fn apply(self) {
        match self {
            Self::Auto => colored::control::unset_override(),
            Self::Always => colored::control::set_override(true),
            Self::Never => colored::control::set_override(false),
        }
    }
}

impl OutputFormat {
    pub(super) fn check(
        self,
        loaded: &LoadedSystem,
        findings: &[Finding],
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(report::check(loaded, findings).to_string()),
            Self::Json | Self::Jsonl => output_json::serialize(&Diagnosis::new(loaded, findings)),
        }
    }

    pub(super) fn catalogue(
        self,
        loaded: &LoadedSystem,
        definition: Option<&str>,
    ) -> Result<String, human_errors::Error> {
        let Some(id) = definition else {
            return match self {
                Self::Table => Ok(report::catalogue(loaded).to_string()),
                Self::Json | Self::Jsonl => output_json::serialize(&loaded.component_types),
            };
        };

        match self {
            Self::Table => report::component_type(loaded, id)
                .map(|report| report.to_string())
                .ok_or_else(|| unknown_definition(id)),
            Self::Json | Self::Jsonl => {
                match (loaded.component_types.get(id), loaded.mutators.get(id)) {
                    (Some(component_type), _) => output_json::serialize(component_type),
                    (None, Some(mutator)) => output_json::serialize(mutator),
                    (None, None) => Err(unknown_definition(id)),
                }
            }
        }
    }

    pub(super) fn solved(
        self,
        loaded: &LoadedSystem,
        evaluation: &Evaluation,
        component: Option<&str>,
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(report::channels(loaded, evaluation, component).to_string()),
            Self::Json | Self::Jsonl => {
                output_json::serialize(&report::channel_values(evaluation, component))
            }
        }
    }

    pub(super) fn bottlenecks(self, ranked: &[Bottleneck]) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(report::bottlenecks(ranked).to_string()),
            Self::Json => output_json::serialize(ranked),
            Self::Jsonl => output_json::lines(ranked),
        }
    }

    pub(super) fn comparison(
        self,
        compared: &[(String, Comparison)],
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(report::comparison(compared).to_string()),
            Self::Json => output_json::serialize(&movements(compared)),
            Self::Jsonl => output_json::lines(&movements(compared)),
        }
    }
}

/// Flattens every comparison into one list naming the proposal each row is from.
///
/// A script weighing several proposals wants them side by side rather than
/// nested, and the intervention a movement belongs to has to travel with it
/// once they share a list.
fn movements(compared: &[(String, Comparison)]) -> Vec<serde_json::Value> {
    compared
        .iter()
        .flat_map(|(intervention, comparison)| {
            comparison.movements.iter().map(move |movement| {
                serde_json::json!({
                    "intervention": intervention,
                    "component": movement.component,
                    "constraint": movement.constraint,
                    "before": movement.before,
                    "after": movement.after,
                    "bound_before": movement.bound_before,
                    "bound_after": movement.bound_after,
                    "relieved": movement.relieved(),
                    "introduced": movement.introduced(),
                })
            })
        })
        .collect()
}

fn unknown_definition(id: &str) -> human_errors::Error {
    human_errors::user(
        format!("This design has no component type or behaviour called `{id}`."),
        &[
            "Run `optimist catalogue` to list everything the design can use.",
            "A project-local definition lives in component-types/ or mutators/ beside the design.",
        ],
    )
}
