use clap::{Args, Subcommand};

use crate::domain::{EdgeId, NewObservation, ProjectId};

use super::{client::ProjectClient, output::OutputFormat};

#[derive(Debug, Args)]
pub(super) struct ObserveArgs {
    #[command(subcommand)]
    command: ObserveCommand,
}

#[derive(Debug, Subcommand)]
enum ObserveCommand {
    Add {
        measurement_edge: EdgeId,
        value: f64,
        #[arg(long)]
        unit: String,
        #[arg(long)]
        observed_at: String,
        #[arg(long)]
        source: String,
        #[arg(long)]
        standard_deviation: Option<f64>,
    },
    Correct {
        measurement_edge: EdgeId,
        observation_id: u64,
        value: f64,
    },
    List {
        measurement_edge: EdgeId,
    },
}

pub(super) async fn run(
    args: ObserveArgs,
    project: Option<&ProjectId>,
    server_url: &str,
    output: OutputFormat,
) -> Result<(), human_errors::Error> {
    let project = project.ok_or_else(|| {
        human_errors::user(
            "A project is required for observation commands.",
            &["Pass `--project <ID>` or set `OPTIMIST_PROJECT` after running `optimist project list`."],
        )
    })?;
    let client = ProjectClient::new(server_url)?;
    let rendered = match args.command {
        ObserveCommand::Add {
            measurement_edge,
            value,
            unit,
            observed_at,
            source,
            standard_deviation,
        } => output.observation(
            &client
                .append_observation(
                    project,
                    measurement_edge,
                    NewObservation {
                        value,
                        unit,
                        observed_at,
                        source,
                        measurement_standard_deviation: standard_deviation,
                    },
                )
                .await?,
        )?,
        ObserveCommand::Correct {
            measurement_edge,
            observation_id,
            value,
        } => output.observation(
            &client
                .correct_observation(project, measurement_edge, observation_id, value)
                .await?,
        )?,
        ObserveCommand::List { measurement_edge } => {
            output.observations(&client.list_observations(project, &measurement_edge).await?)?
        }
    };
    println!("{rendered}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cli::Cli;

    #[test]
    fn parses_observation_uncertainty_fields() {
        assert!(
            Cli::try_parse_from([
                "optimist",
                "--project",
                "A",
                "observe",
                "add",
                "A-measures-B",
                "0.9",
                "--unit",
                "ratio",
                "--observed-at",
                "2026-07-15T12:00:00Z",
                "--source",
                "dashboard",
                "--standard-deviation",
                "0.02"
            ])
            .is_ok()
        );
    }
}
