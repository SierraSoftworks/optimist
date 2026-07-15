use clap::Args;

#[derive(Debug, Args)]
pub(super) struct ApplyArgs {
    command: String,
    #[arg(long)]
    dry_run: bool,
}

pub(super) fn run(_args: ApplyArgs) -> Result<(), human_errors::Error> {
    super::unavailable(
        "Natural-language graph commands are not available in this build yet.",
        &[
            "Use the typed node and edge commands shown by `optimist --help` until `optimist apply` is implemented.",
        ],
    )
}
