use clap::{Args, Subcommand};

use crate::domain::{ProjectDependenceModel, ProjectId};

use super::{client::ProjectClient, output::OutputFormat};

#[derive(Debug, Args)]
#[command(after_long_help = r#"EXAMPLES:
    optimist --project A dependence set --document '{"revision":0,"residual_groups":[{"members":[...],"correlation":{"scale":"rank","matrix":[[1,0.5],[0.5,1]]}}]}'
    optimist --project A dependence show
    optimist --project A dependence remove --revision 0

The document is typed JSON. Matrix row and column order must match each group's member order."#)]
pub(super) struct DependenceArgs {
    #[command(subcommand)]
    command: DependenceCommand,
}

#[derive(Debug, Subcommand)]
enum DependenceCommand {
    /// Create or replace the project dependence document from JSON.
    Set {
        /// Complete ProjectDependenceModel JSON with the current document revision.
        #[arg(long)]
        document: String,
    },
    /// Show the project dependence document.
    Show,
    /// Remove the document using its current revision.
    Remove {
        /// Current dependence document revision.
        #[arg(long)]
        revision: u64,
    },
}

pub(super) async fn run(
    args: DependenceArgs,
    project: Option<&ProjectId>,
    server_url: &str,
    output: OutputFormat,
) -> Result<(), human_errors::Error> {
    let project = project.ok_or_else(|| {
        human_errors::user(
            "A project is required for dependence commands.",
            &["Pass `--project <ID>` or set `OPTIMIST_PROJECT` after running `optimist project list`."],
        )
    })?;
    let client = ProjectClient::new(server_url)?;
    let model = match args.command {
        DependenceCommand::Set { document } => {
            client
                .set_dependence(project, parse_document(&document)?)
                .await?
        }
        DependenceCommand::Show => client.show_dependence(project).await?,
        DependenceCommand::Remove { revision } => {
            client.remove_dependence(project, revision).await?
        }
    };
    println!("{}", output.dependence(&model)?);
    Ok(())
}

fn parse_document(value: &str) -> Result<ProjectDependenceModel, human_errors::Error> {
    serde_json::from_str(value).map_err(|error| {
        human_errors::wrap_user(
            error,
            "The dependence document is not valid ProjectDependenceModel JSON.",
            &["Provide revision and non-overlapping residual_groups with ordered members and square correlation matrices."],
        )
    })
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use tokio::net::TcpListener;

    use crate::{cli::Cli, server};

    use super::{DependenceArgs, DependenceCommand, ProjectClient, run};

    const DOCUMENT: &str = r#"{"revision":0,"residual_groups":[]}"#;

    #[test]
    fn parses_set_show_and_remove_commands() {
        assert!(
            Cli::try_parse_from([
                "optimist",
                "--project",
                "A",
                "dependence",
                "set",
                "--document",
                DOCUMENT
            ])
            .is_ok()
        );
        assert!(Cli::try_parse_from(["optimist", "--project", "A", "dependence", "show"]).is_ok());
        assert!(
            Cli::try_parse_from([
                "optimist",
                "--project",
                "A",
                "dependence",
                "remove",
                "--revision",
                "0"
            ])
            .is_ok()
        );
    }

    #[tokio::test]
    async fn performs_dependence_lifecycle_through_cli_dispatch() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, server::router()).await.unwrap();
        });
        let server_url = format!("http://{address}");
        let client = ProjectClient::new(&server_url).unwrap();
        let project = client.create("Delivery".to_owned()).await.unwrap();
        for command in [
            DependenceCommand::Set {
                document: DOCUMENT.to_owned(),
            },
            DependenceCommand::Show,
            DependenceCommand::Remove { revision: 0 },
        ] {
            run(
                DependenceArgs { command },
                Some(&project.id),
                &server_url,
                crate::cli::output::OutputFormat::Json,
            )
            .await
            .unwrap();
        }
        let error = client.show_dependence(&project.id).await.unwrap_err();
        assert!(
            error
                .advice()
                .iter()
                .any(|item| item.contains("dependence set"))
        );
        server.abort();
    }
}
