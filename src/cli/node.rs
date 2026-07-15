use clap::{Args, Subcommand, ValueEnum};

use crate::domain::{EntityId, ProjectId};

use super::{client::ProjectClient, node_payload, output::OutputFormat};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum NodeType {
    Outcome,
    Metric,
    Factor,
    Intervention,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum Direction {
    Maximize,
    Minimize,
    TargetRange,
}

#[derive(Debug, Args)]
pub(super) struct NodeArgs {
    #[command(subcommand)]
    command: NodeCommand,
}

#[derive(Debug, Subcommand)]
enum NodeCommand {
    Create {
        #[arg(long, value_enum)]
        kind: NodeType,
        #[arg(long)]
        name: String,
        #[arg(long)]
        title: String,
        #[arg(long, value_enum)]
        direction: Option<Direction>,
        #[arg(long)]
        unit: Option<String>,
        #[arg(long)]
        aggregation: Option<String>,
        #[arg(long)]
        controllable: bool,
    },
    Get {
        id: EntityId,
    },
    List,
    Delete {
        id: EntityId,
    },
}

pub(super) async fn run(
    args: NodeArgs,
    project: Option<&ProjectId>,
    server_url: &str,
    output: OutputFormat,
) -> Result<(), human_errors::Error> {
    let project = project.ok_or_else(|| {
        human_errors::user(
            "A project is required for node commands.",
            &["Pass `--project <ID>` or set `OPTIMIST_PROJECT` after running `optimist project list`."],
        )
    })?;
    let client = ProjectClient::new(server_url)?;
    let rendered = match args.command {
        NodeCommand::Create {
            kind,
            name,
            title,
            direction,
            unit,
            aggregation,
            controllable,
        } => {
            let payload = node_payload::build(kind, direction, unit, aggregation, controllable)?;
            output.node(&client.create_node(project, name, title, payload).await?)?
        }
        NodeCommand::Get { id } => output.node(&client.show_node(project, id).await?)?,
        NodeCommand::List => output.nodes(&client.list_nodes(project).await?)?,
        NodeCommand::Delete { id } => output.node(&client.delete_node(project, id).await?)?,
    };
    println!("{rendered}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cli::Cli;

    #[test]
    fn parses_kind_specific_node_fields() {
        assert!(
            Cli::try_parse_from([
                "optimist",
                "--project",
                "A",
                "node",
                "create",
                "--kind",
                "metric",
                "--name",
                "availability",
                "--title",
                "Availability",
                "--unit",
                "ratio"
            ])
            .is_ok()
        );
    }

    #[test]
    fn parses_node_delete_with_typed_id() {
        assert!(Cli::try_parse_from(["optimist", "--project", "A", "node", "delete", "B"]).is_ok());
        assert!(
            Cli::try_parse_from(["optimist", "--project", "A", "node", "delete", "not-an-id"])
                .is_err()
        );
    }
}
