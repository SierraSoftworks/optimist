use clap::{Args, Subcommand};

use clap::ValueEnum;

use crate::domain::{EdgeId, EntityId, ProjectId};

use super::{client::ProjectClient, edge_payload, output::OutputFormat};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum EdgeType {
    Requires,
    PartOf,
    Measures,
    ConflictsWith,
    SynergizesWith,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum RequirementMode {
    Hard,
    Soft,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum Polarity {
    HigherIsBetter,
    LowerIsBetter,
    TargetRange,
}

#[derive(Debug, Args)]
pub(super) struct EdgeArgs {
    #[command(subcommand)]
    command: EdgeCommand,
}

#[derive(Debug, Subcommand)]
enum EdgeCommand {
    Create {
        source: EntityId,
        #[arg(value_enum)]
        kind: EdgeType,
        destination: EntityId,
        #[arg(long, value_enum)]
        requirement: Option<RequirementMode>,
        #[arg(long)]
        threshold: Option<f64>,
        #[arg(long, value_enum)]
        polarity: Option<Polarity>,
    },
    Get {
        id: EdgeId,
    },
    List,
    Delete {
        id: EdgeId,
    },
}

pub(super) async fn run(
    args: EdgeArgs,
    project: Option<&ProjectId>,
    server_url: &str,
    output: OutputFormat,
) -> Result<(), human_errors::Error> {
    let project = project.ok_or_else(|| {
        human_errors::user(
            "A project is required for edge commands.",
            &["Pass `--project <ID>` or set `OPTIMIST_PROJECT` after running `optimist project list`."],
        )
    })?;
    let client = ProjectClient::new(server_url)?;
    let rendered = match args.command {
        EdgeCommand::Create {
            source,
            kind,
            destination,
            requirement,
            threshold,
            polarity,
        } => {
            let payload = edge_payload::build(kind, requirement, threshold, polarity)?;
            output.edge(
                &client
                    .create_edge(project, source, destination, payload)
                    .await?,
            )?
        }
        EdgeCommand::Get { id } => output.edge(&client.show_edge(project, &id).await?)?,
        EdgeCommand::List => output.edges(&client.list_edges(project).await?)?,
        EdgeCommand::Delete { .. } => {
            return super::unavailable(
                "Revision-checked edge deletion is not available yet.",
                &[
                    "Inspect the edge with `optimist edge get` while typed delete commands are implemented.",
                ],
            );
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
    fn parses_kind_specific_edge_fields() {
        assert!(
            Cli::try_parse_from([
                "optimist",
                "--project",
                "A",
                "edge",
                "create",
                "A",
                "requires",
                "B",
                "--requirement",
                "hard"
            ])
            .is_ok()
        );
    }
}
