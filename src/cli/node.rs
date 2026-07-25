use clap::{Args, Subcommand, ValueEnum};

use crate::domain::{EntityId, ProjectId, QuantityDefinition, StateRelation};

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
    /// Replaces the node equation computing a state from its parents.
    ///
    /// Parents bind by node name at the value they held one relationship lag
    /// ago, interventions reaching the state bind their activation, and the
    /// equation replaces proportional composition for this state.
    Relation {
        id: EntityId,
        /// Squiggle source over parent names, activations, `baseline`, and parameters.
        #[arg(long, conflicts_with = "clear")]
        source: Option<String>,
        /// JSON object of named uncertain coefficients keyed by binding name.
        #[arg(long, default_value = "{}")]
        parameters: String,
        /// Removes the equation and restores proportional composition.
        #[arg(long)]
        clear: bool,
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
        NodeCommand::Relation {
            id,
            source,
            parameters,
            clear,
        } => output.node(
            &client
                .set_state_relation(project, id, parse_relation(source, &parameters, clear)?)
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

/// Builds the equation to store, or `None` when the caller is clearing it.
fn parse_relation(
    source: Option<String>,
    parameters: &str,
    clear: bool,
) -> Result<Option<StateRelation>, human_errors::Error> {
    if clear {
        return Ok(None);
    }
    let source = source.ok_or_else(|| {
        human_errors::user(
            "A node equation requires a calculation.",
            &["Pass `--source '<squiggle>'`, or `--clear` to restore proportional composition."],
        )
    })?;
    let parameters = serde_json::from_str(parameters).map_err(|error| {
        human_errors::wrap_user(
            error,
            "Equation parameters are not a valid JSON object of named coefficients.",
            &[
                "Pass `--parameters` an object keyed by binding name, each with `quantity` and `value` fields.",
            ],
        )
    })?;
    StateRelation::new(source, parameters).map(Some).map_err(|error| {
        human_errors::wrap_user(
            error,
            "The node equation could not be prepared.",
            &[
                "Use valid binding names, and keep uncertainty in named parameters rather than in the source.",
            ],
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

    #[test]
    fn parses_node_equations_and_rejects_a_source_beside_clear() {
        assert!(
            Cli::try_parse_from([
                "optimist",
                "--project",
                "A",
                "node",
                "relation",
                "B",
                "--source",
                "outage_frequency * impact_duration",
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "optimist",
                "--project",
                "A",
                "node",
                "relation",
                "B",
                "--clear"
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "optimist",
                "--project",
                "A",
                "node",
                "relation",
                "B",
                "--clear",
                "--source",
                "baseline",
            ])
            .is_err()
        );
    }

    #[test]
    fn requires_a_calculation_unless_the_equation_is_cleared() {
        assert!(super::parse_relation(None, "{}", true).unwrap().is_none());
        assert!(super::parse_relation(None, "{}", false).is_err());
        assert_eq!(
            super::parse_relation(Some("baseline".to_owned()), "{}", false)
                .unwrap()
                .unwrap()
                .source,
            "baseline"
        );
    }
}
