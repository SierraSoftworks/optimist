use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub(super) struct EstimateArgs {
    #[command(subcommand)]
    command: EstimateCommand,
}

#[derive(Debug, Subcommand)]
enum EstimateCommand {
    Set {
        address: String,
        #[arg(long)]
        distribution: String,
    },
    Show {
        address: String,
    },
    Remove {
        address: String,
    },
}

pub(super) fn run(_args: EstimateArgs) -> Result<(), human_errors::Error> {
    super::unavailable(
        "Estimate editing is not available in this build yet.",
        &[
            "Use `optimist estimate --help` to verify the typed estimate address and distribution syntax.",
        ],
    )
}
