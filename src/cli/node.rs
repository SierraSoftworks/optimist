use clap::{Args, Subcommand, ValueEnum};

use crate::domain::{EntityId, ProjectId, QuantityDefinition};

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
    Update {
        id: EntityId,
        /// Complete human-facing title replacement.
        #[arg(long)]
        title: String,
        /// Complete Markdown description replacement.
        #[arg(long, default_value = "")]
        description: String,
        /// Complete JSON object replacement for extensible metadata.
        #[arg(long, default_value = "{}")]
        metadata: String,
    },
    /// Configures a native quantity definition for factor or outcome state.
    Quantity {
        id: EntityId,
        /// Complete QuantityDefinition JSON including canonical dimension and support.
        #[arg(long)]
        definition: String,
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
        NodeCommand::Update {
            id,
            title,
            description,
            metadata,
        } => output.node(
            &client
                .update_node_metadata(project, id, title, description, parse_metadata(&metadata)?)
                .await?,
        )?,
        NodeCommand::Quantity { id, definition } => output.node(
            &client
                .set_node_quantity_state(project, id, parse_quantity_definition(&definition)?)
                .await?,
        )?,
    };
    println!("{rendered}");
    Ok(())
}

fn parse_quantity_definition(value: &str) -> Result<QuantityDefinition, human_errors::Error> {
    serde_json::from_str(value).map_err(|error| {
        human_errors::wrap_user(
            error,
            "The quantity definition is not valid QuantityDefinition JSON.",
            &["Include `unit`, canonical `dimension`, `aggregation`, and `support` fields."],
        )
    })
}

fn parse_metadata(
    value: &str,
) -> Result<std::collections::BTreeMap<String, serde_json::Value>, human_errors::Error> {
    serde_json::from_str(value).map_err(|error| {
        human_errors::wrap_user(
            error,
            "Node metadata is not a valid JSON object.",
            &["Pass `--metadata` a JSON object such as `{\"owner\":\"delivery\"}`."],
        )
    })
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

    #[test]
    fn parses_node_metadata_update() {
        assert!(
            Cli::try_parse_from([
                "optimist",
                "--project",
                "A",
                "node",
                "update",
                "B",
                "--title",
                "Delivery",
                "--description",
                "# Delivery",
                "--metadata",
                r#"{"owner":"team"}"#,
            ])
            .is_ok()
        );
    }

    #[test]
    fn parses_native_quantity_configuration() {
        assert!(
            Cli::try_parse_from([
                "optimist",
                "--project",
                "A",
                "node",
                "quantity",
                "B",
                "--definition",
                r#"{"unit":"days","dimension":{"day":1},"aggregation":null}"#,
            ])
            .is_ok()
        );
    }
}
