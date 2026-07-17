use clap::Subcommand;
use uuid::Uuid;

use crate::domain::ProjectId;

use super::{client::ProjectClient, output::OutputFormat};

#[derive(Debug, Subcommand)]
pub(super) enum BackupCommand {
    Create,
    List,
    Restore {
        backup: Uuid,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum SnapshotCommand {
    Create,
    List,
    Show { revision: u64 },
}

pub(super) async fn run_backup(
    command: BackupCommand,
    client: &ProjectClient,
    output: OutputFormat,
) -> Result<String, human_errors::Error> {
    match command {
        BackupCommand::Create => output.catalog_backup(&client.create_backup().await?),
        BackupCommand::List => output.catalog_backups(&client.list_backups().await?),
        BackupCommand::Restore { backup, yes } => {
            output.catalog_restore(&client.restore_backup(backup, yes).await?)
        }
    }
}

pub(super) async fn run_snapshot(
    command: SnapshotCommand,
    project: &ProjectId,
    client: &ProjectClient,
    output: OutputFormat,
) -> Result<String, human_errors::Error> {
    match command {
        SnapshotCommand::Create => {
            output.project_snapshot(&client.create_project_snapshot(project).await?)
        }
        SnapshotCommand::List => {
            output.project_snapshots(&client.list_project_snapshots(project).await?)
        }
        SnapshotCommand::Show { revision } => {
            output.project_archive(&client.get_project_snapshot(project, revision).await?)
        }
    }
}
