use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct ObserveArgs {
    #[command(subcommand)]
    pub command: ObserveCommand,
}

#[derive(Debug, Subcommand)]
pub enum ObserveCommand {
    Add {
        measurement_edge: String,
        value: f64,
        #[arg(long)]
        unit: String,
        #[arg(long)]
        observed_at: String,
        #[arg(long)]
        source: String,
        #[arg(long)]
        standard_deviation: Option<f64>,
    },
    Correct {
        measurement_edge: String,
        observation_id: String,
        value: f64,
    },
    List {
        measurement_edge: String,
    },
}

pub(super) fn run(_args: ObserveArgs) -> Result<(), human_errors::Error> {
    super::unavailable(
        "Observation editing is not available in this build yet.",
        &[
            "Use `optimist observe --help` to verify the measurement edge and required uncertainty fields.",
        ],
    )
}
