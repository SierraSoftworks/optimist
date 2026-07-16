use clap::{Args, Subcommand};

use crate::domain::{EstimateAddress, ProjectId};

use super::{client::ProjectClient, output::OutputFormat};

#[derive(Debug, Args)]
#[command(after_long_help = r#"EXAMPLES:
    optimist --project A formula set A/node/B/estimate/A/component/baseline --formula '{"type":"reference","address":{"project":"A","owner":{"kind":"node","id":"B"},"estimate":"A"}}'
    optimist --project A formula list
    optimist --project A formula show A/node/B/estimate/A/component/baseline
    optimist --project A formula remove A/node/B/estimate/A/component/baseline

Formula targets must be nested component addresses under an existing primitive estimate. Literals carry explicit units; references resolve against primitive roots and stored components in the selected project."#)]
pub(super) struct FormulaArgs {
    #[command(subcommand)]
    command: FormulaCommand,
}

#[derive(Debug, Subcommand)]
enum FormulaCommand {
    /// Create or replace one validated nested Fermi component.
    Set {
        address: EstimateAddress,
        /// Tagged Formula JSON.
        #[arg(long)]
        formula: String,
        /// JSON array of evidence or elicitation strings.
        #[arg(long, default_value = "[]")]
        provenance: String,
    },
    /// Show one compiled formula definition.
    Show { address: EstimateAddress },
    /// List all formulas and the current formula document revision.
    List,
    /// Remove one unreferenced leaf component.
    Remove { address: EstimateAddress },
}

pub(super) async fn run(
    args: FormulaArgs,
    project: Option<&ProjectId>,
    server_url: &str,
    output: OutputFormat,
) -> Result<(), human_errors::Error> {
    let project = project.ok_or_else(|| {
        human_errors::user(
            "A project is required for formula commands.",
            &["Pass `--project <ID>` or set `OPTIMIST_PROJECT` after running `optimist project list`."],
        )
    })?;
    let client = ProjectClient::new(server_url)?;
    let rendered = match args.command {
        FormulaCommand::Set {
            address,
            formula,
            provenance,
        } => output.formula(
            &client
                .set_formula(
                    project,
                    address,
                    parse_json(&formula, "Formula")?,
                    parse_json(&provenance, "provenance string array")?,
                )
                .await?,
        )?,
        FormulaCommand::Show { address } => {
            output.formula(&client.show_formula(project, &address).await?)?
        }
        FormulaCommand::List => output.formulas(&client.list_formulas(project).await?)?,
        FormulaCommand::Remove { address } => {
            output.formula(&client.remove_formula(project, address).await?)?
        }
    };
    println!("{rendered}");
    Ok(())
}

fn parse_json<T: serde::de::DeserializeOwned>(
    value: &str,
    expected: &'static str,
) -> Result<T, human_errors::Error> {
    serde_json::from_str(value).map_err(|error| {
        human_errors::wrap_user(
            error,
            format!("The formula input is not valid {expected} JSON."),
            &["Run `optimist formula --help` for component address and formula examples."],
        )
    })
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cli::Cli;

    #[test]
    fn parses_formula_lifecycle_commands() {
        for arguments in [
            vec![
                "optimist",
                "--project",
                "A",
                "formula",
                "set",
                "A/node/B/estimate/A/component/base",
                "--formula",
                r#"{"type":"literal","distribution":{"type":"point","value":1},"unit":{}}"#,
            ],
            vec!["optimist", "--project", "A", "formula", "list"],
            vec![
                "optimist",
                "--project",
                "A",
                "formula",
                "show",
                "A/node/B/estimate/A/component/base",
            ],
            vec![
                "optimist",
                "--project",
                "A",
                "formula",
                "remove",
                "A/node/B/estimate/A/component/base",
            ],
        ] {
            Cli::try_parse_from(arguments).unwrap();
        }
    }
}
