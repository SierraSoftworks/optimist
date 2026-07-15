use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub(super) struct ScenarioArgs {
    #[command(subcommand)]
    command: ScenarioCommand,
}

#[derive(Debug, Subcommand)]
enum ScenarioCommand {
    Create { name: String },
    Show { id: String },
    List,
    Analyze { id: String },
}

pub(super) fn run(_args: ScenarioArgs) -> Result<(), human_errors::Error> {
    super::unavailable(
        "Scenario management is not available in this build yet.",
        &["Create the project graph first, then retry after scenario API support is implemented."],
    )
}
