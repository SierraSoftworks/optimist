use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::domain::ProjectId;

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

pub(super) fn run(_args: ProjectArgs) -> Result<(), human_errors::Error> {
    super::unavailable(
        "Project management is not available in this build yet.",
        &[
            "Start an Optimist server once server support is implemented, then retry this project command.",
        ],
    )
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
