use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::domain::ProjectId;

use super::{client::ProjectClient, output::OutputFormat};

#[derive(Debug, Args)]
pub(super) struct ProjectArgs {
    #[command(subcommand)]
    command: ProjectCommand,
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    Create {
        name: String,
    },
    List,
    Show {
        project: ProjectId,
    },
    Delete {
        project: ProjectId,
    },
    Import {
        directory: PathBuf,
        #[arg(long)]
        replace: bool,
        #[arg(long, requires = "replace")]
        yes: bool,
    },
    Export {
        directory: PathBuf,
    },
}

pub(super) async fn run(
    args: ProjectArgs,
    server_url: &str,
    output: OutputFormat,
) -> Result<(), human_errors::Error> {
    let client = ProjectClient::new(server_url)?;
    let rendered = match args.command {
        ProjectCommand::Create { name } => output.project(&client.create(name).await?)?,
        ProjectCommand::List => output.projects(&client.list().await?)?,
        ProjectCommand::Show { project } => output.project(&client.show(&project).await?)?,
        ProjectCommand::Delete { project } => output.project(&client.delete(&project).await?)?,
        ProjectCommand::Import { .. } => {
            return super::unavailable(
                "Markdown project import is not available yet.",
                &[
                    "Use project create/list/show/delete while the validated Markdown import pipeline is implemented.",
                ],
            );
        }
        ProjectCommand::Export { .. } => {
            return super::unavailable(
                "Markdown project export is not available yet.",
                &[
                    "Use project list/show to inspect project metadata while deterministic Markdown export is implemented.",
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

    use crate::cli::{Cli, Command};

    use super::{ProjectArgs, ProjectCommand};

    #[test]
    fn parses_project_import() {
        let cli = Cli::try_parse_from([
            "optimist",
            "--project",
            "delivery",
            "project",
            "import",
            "./model",
        ])
        .expect("parse project import");
        assert!(matches!(
            cli.command,
            Command::Project(ProjectArgs {
                command: ProjectCommand::Import { replace: false, .. }
            })
        ));
    }

    #[test]
    fn replacement_requires_confirmation_flag() {
        let result = Cli::try_parse_from(["optimist", "project", "import", "./model", "--yes"]);
        assert!(result.is_err());
    }
}
