use clap::{Args, Subcommand, ValueEnum};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum NodeType {
    Outcome,
    Metric,
    Factor,
    Intervention,
}

#[derive(Debug, Args)]
pub(super) struct NodeArgs {
    #[command(subcommand)]
    command: NodeCommand,
}

#[derive(Debug, Subcommand)]
enum NodeCommand {
    Create {
        #[arg(long, value_enum)]
        kind: NodeType,
        #[arg(long)]
        name: String,
        #[arg(long)]
        title: String,
    },
    Get {
        id: String,
    },
    List,
    Delete {
        id: String,
    },
}

pub(super) fn run(_args: NodeArgs) -> Result<(), human_errors::Error> {
    super::unavailable(
        "Node editing is not available in this build yet.",
        &[
            "Use `optimist node --help` to verify the command syntax and retry after the graph API is implemented.",
        ],
    )
}
