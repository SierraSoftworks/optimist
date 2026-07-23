use clap::{Args, Subcommand};

use crate::domain::{EstimateAddress, ProjectId};

use super::{client::ProjectClient, output::OutputFormat};

#[derive(Debug, Args)]
#[command(after_long_help = r#"EXAMPLES:
    optimist --project A estimate set A/node/B/estimate/A --slot '{"kind":"current"}' --definition '{"source":"beta(2, 3)","seed":42,"sample_count":2048,"target_unit":{}}'
    optimist --project A estimate show A/node/B/estimate/A
    optimist --project A estimate remove A/node/B/estimate/A

Slots are current, forecast, cost, duration, probability_of_success, response, lag, or degree. Squiggle support must fit the slot's typed dimension."#)]
pub(super) struct EstimateArgs {
    #[command(subcommand)]
    command: EstimateCommand,
}

#[derive(Debug, Subcommand)]
enum EstimateCommand {
    /// Create or replace one primitive estimate in a typed owner field.
    Set {
        address: EstimateAddress,
        /// Tagged EstimateSlot JSON.
        #[arg(long)]
        slot: String,
        /// SquiggleEstimateDefinition JSON.
        #[arg(long)]
        definition: String,
        /// JSON array of evidence or elicitation strings.
        #[arg(long, default_value = "[]")]
        provenance: String,
        /// EstimateUncertainty JSON with epistemic, process, and measurement fields.
        #[arg(long, default_value = "{}")]
        uncertainty: String,
    },
    /// Show one primitive estimate by canonical address.
    Show { address: EstimateAddress },
    /// Remove one optional or named-cost estimate by canonical address.
    Remove { address: EstimateAddress },
}

pub(super) async fn run(
    args: EstimateArgs,
    project: Option<&ProjectId>,
    server_url: &str,
    output: OutputFormat,
) -> Result<(), human_errors::Error> {
    let project = project.ok_or_else(|| {
        human_errors::user(
            "A project is required for estimate commands.",
            &["Pass `--project <ID>` or set `OPTIMIST_PROJECT` after running `optimist project list`."],
        )
    })?;
    let client = ProjectClient::new(server_url)?;
    let value = match args.command {
        EstimateCommand::Set {
            address,
            slot,
            definition,
            provenance,
            uncertainty,
        } => {
            client
                .set_squiggle_estimate(
                    project,
                    address,
                    parse_json(&slot, "EstimateSlot")?,
                    parse_json(&definition, "SquiggleEstimateDefinition")?,
                    parse_json(&provenance, "provenance string array")?,
                    parse_json(&uncertainty, "EstimateUncertainty")?,
                )
                .await?
        }
        EstimateCommand::Show { address } => client.show_estimate(project, &address).await?,
        EstimateCommand::Remove { address } => client.remove_estimate(project, address).await?,
    };
    println!("{}", output.estimate(&value)?);
    Ok(())
}

fn parse_json<T: serde::de::DeserializeOwned>(
    value: &str,
    expected: &'static str,
) -> Result<T, human_errors::Error> {
    serde_json::from_str(value).map_err(|error| {
        human_errors::wrap_user(
            error,
            format!("The estimate input is not valid {expected} JSON."),
            &["Run `optimist estimate --help` for typed slot and Squiggle examples."],
        )
    })
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cli::Cli;

    #[test]
    fn parses_structured_estimate_lifecycle_commands() {
        for arguments in [
            vec![
                "optimist",
                "--project",
                "A",
                "estimate",
                "set",
                "A/node/B/estimate/A",
                "--slot",
                r#"{"kind":"current"}"#,
                "--definition",
                r#"{"source":"beta(2, 3)","seed":42,"sample_count":2048,"target_unit":{}}"#,
            ],
            vec![
                "optimist",
                "--project",
                "A",
                "estimate",
                "show",
                "A/node/B/estimate/A",
            ],
            vec![
                "optimist",
                "--project",
                "A",
                "estimate",
                "remove",
                "A/node/B/estimate/A",
            ],
        ] {
            Cli::try_parse_from(arguments).unwrap();
        }
    }
}
