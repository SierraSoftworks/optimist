use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub(super) struct EdgeArgs {
    #[command(subcommand)]
    command: EdgeCommand,
}

#[derive(Debug, Subcommand)]
enum EdgeCommand {
    Create {
        source: String,
        kind: String,
        destination: String,
    },
    Get {
        id: String,
    },
    List,
    Delete {
        id: String,
    },
}

pub(super) fn run(_args: EdgeArgs) -> Result<(), human_errors::Error> {
    super::unavailable(
        "Edge editing is not available in this build yet.",
        &[
            "Use `optimist edge --help` to verify the command syntax and retry after the graph API is implemented.",
        ],
    )
}
