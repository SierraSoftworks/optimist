mod apply;
mod edge;
mod estimate;
mod node;
mod observe;
mod output;
mod project;
mod scenario;
mod server;

use clap::{Parser, Subcommand};

pub use apply::ApplyArgs;
pub use edge::EdgeArgs;
pub use estimate::EstimateArgs;
pub use node::NodeArgs;
pub use observe::ObserveArgs;
pub use output::OutputFormat;
pub use project::ProjectArgs;
pub use scenario::ScenarioArgs;
pub use server::ServerArgs;

use crate::domain::ProjectId;

#[derive(Debug, Parser)]
#[command(
    name = "optimist",
    version,
    about = "Model and analyze complex systems"
)]
pub struct Cli {
    #[arg(
        long,
        global = true,
        env = "OPTIMIST_SERVER",
        default_value = "http://127.0.0.1:3000"
    )]
    pub server_url: String,
    #[arg(long, global = true, env = "OPTIMIST_PROJECT")]
    pub project: Option<ProjectId>,
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Table)]
    pub output: OutputFormat,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Server(ServerArgs),
    Project(ProjectArgs),
    Node(NodeArgs),
    Edge(EdgeArgs),
    Observe(ObserveArgs),
    Estimate(EstimateArgs),
    Scenario(ScenarioArgs),
    Apply(ApplyArgs),
}

pub async fn run(cli: Cli) -> Result<(), human_errors::Error> {
    match cli.command {
        Command::Server(args) => server::run(args).await,
        Command::Project(args) => project::run(args),
        Command::Node(args) => node::run(args),
        Command::Edge(args) => edge::run(args),
        Command::Observe(args) => observe::run(args),
        Command::Estimate(args) => estimate::run(args),
        Command::Scenario(args) => scenario::run(args),
        Command::Apply(args) => apply::run(args),
    }
}

pub(super) fn unavailable(
    message: &'static str,
    advice: &'static [&'static str],
) -> Result<(), human_errors::Error> {
    Err(human_errors::user(message, advice))
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Command};

    #[test]
    fn parses_global_agent_output_options() {
        let cli = Cli::try_parse_from([
            "optimist",
            "--project",
            "delivery",
            "--output",
            "json",
            "node",
            "list",
        ])
        .expect("parse global options");
        assert!(matches!(cli.command, Command::Node(_)));
    }
}
