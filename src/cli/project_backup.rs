use clap::Subcommand;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::{
    domain::ProjectId,
    markdown::{RenderedSnapshot, write_directory},
    project::ProjectArchive,
};

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
    Show {
        revision: u64,
    },
    /// Atomically publishes one retained immutable revision as a Markdown directory.
    Export {
        revision: u64,
        directory: PathBuf,
    },
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
        SnapshotCommand::Export {
            revision,
            directory,
        } => {
            let archive = client.get_project_snapshot(project, revision).await?;
            publish_archive(&archive, &directory)?;
            output.project_archive(&archive)
        }
    }
}

pub(super) fn publish_archive(
    archive: &ProjectArchive,
    directory: &Path,
) -> Result<(), human_errors::Error> {
    let import = archive.validated_import().map_err(|error| {
        human_errors::wrap_system(
            error,
            "The Optimist server returned an invalid project archive.",
            &["Confirm the CLI and server versions match, then inspect the server logs."],
        )
    })?;
    let snapshot = RenderedSnapshot::from_import(&import).map_err(|error| {
        human_errors::wrap_system(
            error,
            "Optimist could not render the exported project archive.",
            &["Confirm the CLI and server versions match, then retry the export."],
        )
    })?;
    write_directory(directory, &snapshot).map_err(|error| {
        human_errors::wrap_system(
            error,
            "Optimist could not publish the Markdown export directory.",
            &["Check directory permissions and retry with a writable destination."],
        )
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use uuid::Uuid;

    use crate::{
        command::{CommandRequest, CreateNode, GraphCommand},
        domain::{Factor, NodePayload},
        markdown::{RenderedSnapshot, read_directory},
        project::{CatalogStore, ProjectCatalog},
    };

    use super::publish_archive;

    #[test]
    fn publishes_immutable_archives_byte_stably_and_removes_stale_files() {
        let root = std::env::temp_dir().join(format!(
            "optimist-snapshot-directory-export-{}",
            Uuid::new_v4()
        ));
        let store = CatalogStore::new(root.join("catalog"));
        let export = root.join("export");
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    0,
                    GraphCommand::CreateNode(CreateNode {
                        name: "flow".to_owned(),
                        title: "Flow".to_owned(),
                        payload: NodePayload::Factor(Factor {
                            current: None,
                            desired: None,
                            controllable: false,
                            evidence: vec![],
                        }),
                    }),
                ),
            )
            .unwrap();
        let retained = store
            .create_project_snapshot(&mut catalog, &project.id)
            .unwrap();
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    retained.revision,
                    GraphCommand::CreateNode(CreateNode {
                        name: "later".to_owned(),
                        title: "Later".to_owned(),
                        payload: NodePayload::Factor(Factor {
                            current: None,
                            desired: None,
                            controllable: false,
                            evidence: vec![],
                        }),
                    }),
                ),
            )
            .unwrap();
        let archive = store
            .get_project_snapshot(&project.id, retained.revision)
            .unwrap();
        let current = catalog.export_archive(&project.id).unwrap();
        assert_ne!(archive.files, current.files);

        publish_archive(&archive, &export).unwrap();
        let first = RenderedSnapshot::from_import(&read_directory(&export).unwrap()).unwrap();
        fs::write(export.join("stale.md"), "stale").unwrap();
        publish_archive(&archive, &export).unwrap();
        let second = RenderedSnapshot::from_import(&read_directory(&export).unwrap()).unwrap();

        assert_eq!(second, first);
        assert!(!export.join("stale.md").exists());
        assert_eq!(
            archive.files,
            second
                .files()
                .map(|(path, contents)| (path.to_owned(), contents.to_owned()))
                .collect()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
