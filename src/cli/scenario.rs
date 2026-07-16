use clap::{Args, Subcommand};

use crate::domain::{ProjectId, ScenarioDraft, ScenarioId};

use super::{client::ProjectClient, output::OutputFormat};

#[derive(Debug, Args)]
#[command(after_long_help = r#"EXAMPLES:
    optimist --project A scenario create --document '{"name":"delivery","title":"Delivery plan","rationale":"Prefer sustainable gains.","objectives":[{"outcome_id":"A","direction":"maximize","importance":1.0}],"planning_horizon":12,"budgets":[{"unit":{"usd":1},"amount":50000}],"candidate_interventions":["B"],"monte_carlo":{"seed":42,"minimum_samples":1000,"maximum_samples":100000,"absolute_tolerance":0.001,"relative_tolerance":0.01}}'

    optimist --project A scenario update A --revision 0 --document '<ScenarioDraft JSON>'
    optimist --project A scenario delete A --revision 1

Structured JSON is required so objectives, units, budgets, candidates, sampling controls, and optional scalar_preferences retain their typed schema."#)]
pub(super) struct ScenarioArgs {
    #[command(subcommand)]
    command: ScenarioCommand,
}

#[derive(Debug, Subcommand)]
enum ScenarioCommand {
    /// Create a scenario from one structured JSON document.
    Create {
        /// ScenarioDraft JSON, including objectives, budgets, candidates, and sampling controls.
        #[arg(long)]
        document: String,
    },
    /// Show one scenario document.
    Show { id: ScenarioId },
    /// List scenario documents in stable project-local ID order.
    List,
    /// Replace one scenario document using its current document revision.
    Update {
        id: ScenarioId,
        #[arg(long)]
        revision: u64,
        #[arg(long)]
        document: String,
    },
    /// Delete one scenario document using its current document revision.
    Delete {
        id: ScenarioId,
        #[arg(long)]
        revision: u64,
    },
    /// Analyze a scenario once causal decision analysis is available.
    Analyze { id: ScenarioId },
}

pub(super) async fn run(
    args: ScenarioArgs,
    project: Option<&ProjectId>,
    server_url: &str,
    output: OutputFormat,
) -> Result<(), human_errors::Error> {
    if matches!(&args.command, ScenarioCommand::Analyze { .. }) {
        return super::unavailable(
            "Scenario analysis is not available in this build yet.",
            &[
                "Scenario documents can be managed now. Analysis computation remains intentionally unavailable until the causal projection is implemented.",
            ],
        );
    }
    let project = project.ok_or_else(|| {
        human_errors::user(
            "A project is required for scenario commands.",
            &["Pass `--project <ID>` or set `OPTIMIST_PROJECT` after running `optimist project list`."],
        )
    })?;
    let client = ProjectClient::new(server_url)?;
    let rendered = match args.command {
        ScenarioCommand::Create { document } => output.scenario(
            &client
                .create_scenario(project, parse_document(&document)?)
                .await?,
        )?,
        ScenarioCommand::Show { id } => {
            output.scenario(&client.show_scenario(project, id).await?)?
        }
        ScenarioCommand::List => output.scenarios(&client.list_scenarios(project).await?)?,
        ScenarioCommand::Update {
            id,
            revision,
            document,
        } => output.scenario(
            &client
                .update_scenario(project, id, revision, parse_document(&document)?)
                .await?,
        )?,
        ScenarioCommand::Delete { id, revision } => {
            output.scenario(&client.delete_scenario(project, id, revision).await?)?
        }
        ScenarioCommand::Analyze { .. } => unreachable!("handled before project resolution"),
    };
    println!("{rendered}");
    Ok(())
}

fn parse_document(value: &str) -> Result<ScenarioDraft, human_errors::Error> {
    serde_json::from_str(value).map_err(|error| {
        human_errors::wrap_user(
            error,
            "The scenario document is not valid ScenarioDraft JSON.",
            &[
                "Pass `--document` a JSON object containing name, title, rationale, objectives, planning_horizon, budgets, candidate_interventions, and monte_carlo.",
                "Use `scalar_preferences` only when explicit scalar utility conversion is intended.",
            ],
        )
    })
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use tokio::net::TcpListener;

    use crate::{cli::Cli, domain::ScenarioId, server};

    use super::{ProjectClient, ScenarioArgs, ScenarioCommand, run};

    const DOCUMENT: &str = r#"{"name":"delivery","title":"Delivery","rationale":"Choose an investment.","objectives":[{"outcome_id":"A","direction":"maximize","importance":1.0}],"planning_horizon":4,"budgets":[],"candidate_interventions":["B"],"monte_carlo":{"seed":7,"minimum_samples":10,"maximum_samples":100,"absolute_tolerance":0.01,"relative_tolerance":0.01}}"#;
    const EMPTY_DOCUMENT: &str = r#"{"name":"delivery","title":"Delivery","rationale":"Choose an investment.","objectives":[],"planning_horizon":4,"budgets":[],"candidate_interventions":[],"monte_carlo":{"seed":7,"minimum_samples":10,"maximum_samples":100,"absolute_tolerance":0.01,"relative_tolerance":0.01}}"#;
    const UPDATED_DOCUMENT: &str = r#"{"name":"delivery","title":"Reliable delivery","rationale":"Choose an investment.","objectives":[],"planning_horizon":8,"budgets":[],"candidate_interventions":[],"monte_carlo":{"seed":7,"minimum_samples":10,"maximum_samples":100,"absolute_tolerance":0.01,"relative_tolerance":0.01}}"#;

    #[test]
    fn parses_structured_create_update_and_delete_commands() {
        assert!(
            Cli::try_parse_from([
                "optimist",
                "--project",
                "A",
                "scenario",
                "create",
                "--document",
                DOCUMENT,
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "optimist",
                "--project",
                "A",
                "scenario",
                "update",
                "A",
                "--revision",
                "0",
                "--document",
                DOCUMENT,
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "optimist",
                "--project",
                "A",
                "scenario",
                "delete",
                "A",
                "--revision",
                "1",
            ])
            .is_ok()
        );
    }

    #[test]
    fn rejects_noncanonical_scenario_ids() {
        assert!(
            Cli::try_parse_from(["optimist", "--project", "A", "scenario", "show", "AA"]).is_err()
        );
    }

    #[tokio::test]
    async fn creates_scenarios_through_cli_dispatch_and_keeps_analysis_unavailable() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, server::router()).await.unwrap();
        });
        let server_url = format!("http://{address}");
        let client = ProjectClient::new(&server_url).unwrap();
        let project = client.create("Delivery".to_owned()).await.unwrap();

        run(
            ScenarioArgs {
                command: ScenarioCommand::Create {
                    document: EMPTY_DOCUMENT.to_owned(),
                },
            },
            Some(&project.id),
            &server_url,
            crate::cli::output::OutputFormat::Json,
        )
        .await
        .unwrap();
        assert_eq!(client.list_scenarios(&project.id).await.unwrap().len(), 1);

        for command in [
            ScenarioCommand::List,
            ScenarioCommand::Show {
                id: ScenarioId::new(0),
            },
            ScenarioCommand::Update {
                id: ScenarioId::new(0),
                revision: 0,
                document: UPDATED_DOCUMENT.to_owned(),
            },
        ] {
            run(
                ScenarioArgs { command },
                Some(&project.id),
                &server_url,
                crate::cli::output::OutputFormat::Json,
            )
            .await
            .unwrap();
        }
        assert_eq!(
            client
                .show_scenario(&project.id, ScenarioId::new(0))
                .await
                .unwrap()
                .draft
                .planning_horizon,
            8
        );
        run(
            ScenarioArgs {
                command: ScenarioCommand::Delete {
                    id: ScenarioId::new(0),
                    revision: 1,
                },
            },
            Some(&project.id),
            &server_url,
            crate::cli::output::OutputFormat::Json,
        )
        .await
        .unwrap();
        assert!(client.list_scenarios(&project.id).await.unwrap().is_empty());

        let error = run(
            ScenarioArgs {
                command: ScenarioCommand::Analyze {
                    id: ScenarioId::new(0),
                },
            },
            None,
            "not a URL",
            crate::cli::output::OutputFormat::Table,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("analysis is not available"));
        assert!(
            error
                .advice()
                .iter()
                .any(|item| item.contains("intentionally unavailable"))
        );
        server.abort();
    }
}
