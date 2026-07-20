use clap::{Args, Subcommand};
use uuid::Uuid;

use crate::{command::GraphCommand, domain::ProjectId};

use super::{client::ProjectClient, output::OutputFormat};

#[derive(Debug, Args)]
pub(super) struct BatchArgs {
    #[command(subcommand)]
    command: BatchCommand,
}

#[derive(Debug, Subcommand)]
enum BatchCommand {
    Apply {
        #[arg(long)]
        request_id: Uuid,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long, value_parser = parse_commands)]
        commands: CommandList,
    },
    Undo {
        batch: Uuid,
        #[arg(long)]
        request_id: Uuid,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long, value_parser = parse_commands)]
        commands: CommandList,
    },
}

#[derive(Clone, Debug)]
struct CommandList(Vec<GraphCommand>);

pub(super) async fn run(
    args: BatchArgs,
    project: Option<&ProjectId>,
    server_url: &str,
    output: OutputFormat,
) -> Result<(), human_errors::Error> {
    let project = project.ok_or_else(|| {
        human_errors::user(
            "A project is required for command batches.",
            &["Pass `--project <ID>` or set `OPTIMIST_PROJECT`."],
        )
    })?;
    let client = ProjectClient::new(server_url)?;
    let result = match args.command {
        BatchCommand::Apply {
            request_id,
            expected_revision,
            commands,
        } => {
            client
                .execute_batch(project, request_id, expected_revision, commands.0)
                .await?
        }
        BatchCommand::Undo {
            batch,
            request_id,
            expected_revision,
            commands,
        } => {
            client
                .undo_batch(project, batch, request_id, expected_revision, commands.0)
                .await?
        }
    };
    println!("{}", output.command_batch(&result)?);
    Ok(())
}

fn parse_commands(value: &str) -> Result<CommandList, String> {
    serde_json::from_str(value)
        .map(CommandList)
        .map_err(|error| format!("invalid command array: {error}"))
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cli::Cli;

    #[test]
    fn parses_atomic_apply_and_compensating_undo() {
        let commands = r#"[{"type":"delete_node","payload":{"id":"A"}}]"#;
        assert!(
            Cli::try_parse_from([
                "optimist",
                "--project",
                "A",
                "batch",
                "apply",
                "--request-id",
                "00000000-0000-4000-8000-000000000001",
                "--expected-revision",
                "4",
                "--commands",
                commands,
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "optimist",
                "--project",
                "A",
                "batch",
                "undo",
                "00000000-0000-4000-8000-000000000001",
                "--request-id",
                "00000000-0000-4000-8000-000000000002",
                "--expected-revision",
                "5",
                "--commands",
                commands,
            ])
            .is_ok()
        );
    }
}
