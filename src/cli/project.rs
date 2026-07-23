use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::{
    domain::ProjectId,
    project::ProjectArchive,
    project_yaml::{SCHEMA_VERSION, read_directory},
};

use super::{
    client::ProjectClient,
    output::OutputFormat,
    project_backup::{BackupCommand, SnapshotCommand},
    project_changes_output,
};

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
    Changes {
        project: ProjectId,
        /// Replay changes strictly after this project revision.
        #[arg(long, default_value_t = 0)]
        after: u64,
    },
    Import {
        directory: PathBuf,
        #[arg(long)]
        replace: bool,
        #[arg(long, requires = "replace")]
        yes: bool,
    },
    Export {
        project: ProjectId,
        directory: PathBuf,
    },
    Backup {
        #[command(subcommand)]
        command: BackupCommand,
    },
    Snapshot {
        project: ProjectId,
        #[command(subcommand)]
        command: SnapshotCommand,
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
        ProjectCommand::Changes { project, after } => {
            project_changes_output::render(output, &client.replay_changes(&project, after).await?)?
        }
        ProjectCommand::Import {
            directory,
            replace,
            yes,
        } => {
            let import = read_directory(&directory).map_err(|error| {
                human_errors::wrap_user(
                    error,
                    "Optimist could not read the YAML project directory.",
                    &["Correct the reported YAML file and retry the import."],
                )
            })?;
            let archive = ProjectArchive {
                schema_version: SCHEMA_VERSION,
                project: import.project.document.project.clone(),
                description: import.project.document.description.clone(),
                dependence: import.project.document.dependence.clone(),
                entities: import
                    .entities
                    .into_values()
                    .map(|source| source.document)
                    .collect(),
                scenarios: import
                    .scenarios
                    .into_values()
                    .map(|source| source.document)
                    .collect(),
            };
            output.project(&client.import_archive(&archive, replace, yes).await?)?
        }
        ProjectCommand::Export { project, directory } => {
            let archive = client.export_archive(&project).await?;
            super::project_backup::publish_archive(&archive, &directory)?;
            output.project(&client.show(&project).await?)?
        }
        ProjectCommand::Backup { command } => {
            super::project_backup::run_backup(command, &client, output).await?
        }
        ProjectCommand::Snapshot { project, command } => {
            super::project_backup::run_snapshot(command, &project, &client, output).await?
        }
    };
    println!("{rendered}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;

    use clap::Parser;

    use crate::{
        cli::{Cli, Command, client::ProjectClient, output::OutputFormat},
        server,
    };

    use super::{ProjectArgs, ProjectCommand, run};

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
    fn parses_project_export_with_explicit_project() {
        assert!(Cli::try_parse_from(["optimist", "project", "export", "A", "./model"]).is_ok());
    }

    #[test]
    fn parses_immutable_snapshot_directory_export() {
        assert!(
            Cli::try_parse_from([
                "optimist",
                "project",
                "snapshot",
                "A",
                "export",
                "7",
                "./model-r7",
            ])
            .is_ok()
        );
    }

    #[test]
    fn replacement_requires_confirmation_flag() {
        let result = Cli::try_parse_from(["optimist", "project", "import", "./model", "--yes"]);
        assert!(result.is_err());
    }

    #[test]
    fn parses_change_replay_cursor() {
        assert!(
            Cli::try_parse_from(["optimist", "project", "changes", "A", "--after", "42",]).is_ok()
        );
    }

    #[test]
    fn parses_backup_restore_confirmation() {
        assert!(
            Cli::try_parse_from([
                "optimist",
                "project",
                "backup",
                "restore",
                "00000000-0000-4000-8000-000000000001",
                "--yes",
            ])
            .is_ok()
        );
    }

    #[test]
    fn parses_project_snapshot_revision() {
        assert!(
            Cli::try_parse_from(["optimist", "project", "snapshot", "A", "show", "42"]).is_ok()
        );
    }

    #[tokio::test]
    async fn exports_and_imports_yaml_directories_over_http() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, server::router()).await.unwrap();
        });
        let server_url = format!("http://{address}");
        let client = ProjectClient::new(&server_url).unwrap();
        let project = client.create("Delivery".to_owned()).await.unwrap();
        let directory =
            std::env::temp_dir().join(format!("optimist-cli-archive-{}", uuid::Uuid::new_v4()));

        run(
            ProjectArgs {
                command: ProjectCommand::Export {
                    project: project.id.clone(),
                    directory: directory.clone(),
                },
            },
            &server_url,
            OutputFormat::Json,
        )
        .await
        .unwrap();
        assert!(directory.join("_project.yaml").exists());
        client.delete(&project.id).await.unwrap();
        run(
            ProjectArgs {
                command: ProjectCommand::Import {
                    directory: directory.clone(),
                    replace: false,
                    yes: false,
                },
            },
            &server_url,
            OutputFormat::Json,
        )
        .await
        .unwrap();
        assert_eq!(client.show(&project.id).await.unwrap().name, "Delivery");

        std::fs::remove_dir_all(directory).unwrap();
        server.abort();
    }
}
