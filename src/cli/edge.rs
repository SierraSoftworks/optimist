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
    Update {
        id: EdgeId,
        /// Complete Markdown description replacement.
        #[arg(long, default_value = "")]
        description: String,
        /// Complete JSON object replacement for extensible metadata.
        #[arg(long, default_value = "{}")]
        metadata: String,
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
        EdgeCommand::Delete { id } => output.edge(&client.delete_edge(project, id).await?)?,
        EdgeCommand::Update {
            id,
            description,
            metadata,
        } => output.edge(
            &client
                .update_edge_metadata(project, id, description, parse_metadata(&metadata)?)
                .await?,
        )?,
    };
    println!("{rendered}");
    Ok(())
}

fn parse_metadata(
    value: &str,
) -> Result<std::collections::BTreeMap<String, serde_json::Value>, human_errors::Error> {
    serde_json::from_str(value).map_err(|error| {
        human_errors::wrap_user(
            error,
            "Edge metadata is not a valid JSON object.",
            &["Pass `--metadata` a JSON object such as `{\"source\":\"ADR-1\"}`."],
        )
    })
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

    #[test]
    fn parses_edge_delete_with_typed_id() {
        assert!(
            Cli::try_parse_from([
                "optimist",
                "--project",
                "A",
                "edge",
                "delete",
                "B-requires-A"
            ])
            .is_ok()
        );
    }

    #[test]
    fn parses_edge_metadata_update() {
        assert!(
            Cli::try_parse_from([
                "optimist",
                "--project",
                "A",
                "edge",
                "update",
                "A-requires-B",
                "--description",
                "# Dependency",
                "--metadata",
                r#"{"source":"ADR-1"}"#,
            ])
            .is_ok()
        );
    }
}
