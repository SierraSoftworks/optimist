//! Choosing how a report is rendered.

use clap::ValueEnum;

use crate::system::{Bottleneck, Comparison, Evaluation, LoadedSystem};

use super::{output_json, system_output};

/// How a report should be written.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum OutputFormat {
    /// Aligned columns, for reading.
    Table,
    /// One JSON document, for a script that wants the whole answer.
    Json,
    /// One JSON document per line, for a script that streams.
    Jsonl,
}

impl OutputFormat {
    pub(super) fn system_summary(
        self,
        loaded: &LoadedSystem,
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(system_output::summary(loaded)),
            Self::Json | Self::Jsonl => output_json::serialize(&loaded.model),
        }
    }

    pub(super) fn system_catalogue(
        self,
        loaded: &LoadedSystem,
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(system_output::catalogue(loaded)),
            Self::Json | Self::Jsonl => output_json::serialize(&loaded.component_types),
        }
    }

    pub(super) fn system_channels(
        self,
        evaluation: &Evaluation,
        component: Option<&str>,
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(system_output::channels(evaluation, component)),
            Self::Json | Self::Jsonl => {
                output_json::serialize(&system_output::channel_values(evaluation, component))
            }
        }
    }

    pub(super) fn system_bottlenecks(
        self,
        ranked: &[Bottleneck],
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(system_output::bottlenecks(ranked)),
            Self::Json => output_json::serialize(ranked),
            Self::Jsonl => output_json::lines(ranked),
        }
    }

    pub(super) fn system_comparison(
        self,
        comparison: &Comparison,
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(system_output::comparison(comparison)),
            Self::Json | Self::Jsonl => output_json::serialize(&comparison.movements),
        }
    }
}
