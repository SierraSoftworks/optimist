use clap::{Args, Subcommand};

use crate::domain::{AnalysisLimits, ProjectId, ScenarioId, StructuralAnalysis};

use super::{client::ProjectClient, output::OutputFormat};

#[derive(Debug, Args)]
#[command(after_long_help = r#"EXAMPLES:
    optimist --project A analysis structure
    optimist --project A analysis structure --scenario A --maximum-cycle-length 6 --maximum-cycles 500

Structural analysis returns exact causal SCCs and bounded elementary cycles. It does not compute posterior intervention impact or feedback stability."#)]
pub(super) struct AnalysisArgs {
    #[command(subcommand)]
    command: AnalysisCommand,
}

#[derive(Debug, Subcommand)]
enum AnalysisCommand {
    /// Compute exact causal SCCs and bounded elementary cycles.
    Structure {
        /// Optional scenario whose revision should be included in the snapshot key.
        #[arg(long)]
        scenario: Option<ScenarioId>,
        /// Maximum number of causal edges in one returned cycle.
        #[arg(long, default_value_t = 8)]
        maximum_cycle_length: usize,
        /// Maximum number of elementary cycles returned.
        #[arg(long, default_value_t = 1_000)]
        maximum_cycles: usize,
    },
}

pub(super) async fn run(
    args: AnalysisArgs,
    project: Option<&ProjectId>,
    server_url: &str,
    output: OutputFormat,
) -> Result<(), human_errors::Error> {
    let project = project.ok_or_else(|| {
        human_errors::user(
            "A project is required for analysis commands.",
            &["Pass `--project <ID>` or set `OPTIMIST_PROJECT` after running `optimist project list`."],
        )
    })?;
    let client = ProjectClient::new(server_url)?;
    let AnalysisCommand::Structure {
        scenario,
        maximum_cycle_length,
        maximum_cycles,
    } = args.command;
    let limits = AnalysisLimits::new(maximum_cycle_length, maximum_cycles).map_err(|error| {
        human_errors::wrap_user(
            error,
            "The structural analysis limits are invalid.",
            &["Use positive `--maximum-cycle-length` and `--maximum-cycles` values."],
        )
    })?;
    let analysis = client.analyze_structure(project, scenario, limits).await?;
    println!("{}", render(output, &analysis)?);
    Ok(())
}

fn render(
    output: OutputFormat,
    analysis: &StructuralAnalysis,
) -> Result<String, human_errors::Error> {
    match output {
        OutputFormat::Table => Ok(format!(
            "PROJECT\tGRAPH_REVISION\tSCENARIO\tDEPENDENCE_REVISION\tCOMPONENTS\tFEEDBACK_COMPONENTS\tCYCLES\tTRUNCATED\n{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            analysis.revision.project,
            analysis.revision.graph_revision,
            analysis
                .revision
                .scenario
                .map(|(id, revision)| format!("{id}@{revision}"))
                .unwrap_or_else(|| "-".to_owned()),
            analysis
                .revision
                .dependence_revision
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_owned()),
            analysis.components.len(),
            analysis
                .components
                .iter()
                .filter(|value| value.is_feedback)
                .count(),
            analysis.cycles.len(),
            analysis.cycles_truncated,
        )),
        OutputFormat::Json => serde_json::to_string(analysis).map_err(serialization_error),
        OutputFormat::Jsonl => analysis
            .cycles
            .iter()
            .map(|cycle| serde_json::to_string(cycle).map_err(serialization_error))
            .collect::<Result<Vec<_>, _>>()
            .map(|lines| lines.join("\n")),
    }
}

fn serialization_error(error: serde_json::Error) -> human_errors::Error {
    human_errors::wrap_system(
        error,
        "Optimist could not serialize structural analysis output.",
        &["Retry with `--output table` and report the serialization failure if it persists."],
    )
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cli::Cli;

    #[test]
    fn parses_bounded_structural_analysis() {
        Cli::try_parse_from([
            "optimist",
            "--project",
            "A",
            "analysis",
            "structure",
            "--scenario",
            "B",
            "--maximum-cycle-length",
            "6",
            "--maximum-cycles",
            "100",
        ])
        .unwrap();
    }
}
