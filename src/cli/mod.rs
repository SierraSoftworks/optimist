mod apply;
mod client;
mod client_advice;
mod edge;
mod edge_client;
mod edge_payload;
mod estimate;
mod node;
mod node_client;
mod node_payload;
mod observe;
mod output;
mod project;
mod scenario;
mod server;

use clap::{Parser, Subcommand};

use apply::ApplyArgs;
use edge::EdgeArgs;
use estimate::EstimateArgs;
use node::NodeArgs;
use observe::ObserveArgs;
use output::OutputFormat;
use project::ProjectArgs;
use scenario::ScenarioArgs;
use server::ServerArgs;

use crate::domain::ProjectId;

/// Parses the complete `optimist` command line.
///
/// Applications normally construct this value through [`clap::Parser::parse`]
/// and pass it directly to [`run`]. Keeping dispatch behind this opaque type
/// ensures every client command uses the same server/project/output rules.
///
/// ```
/// use clap::Parser;
/// use optimist::cli::Cli;
///
/// let cli = Cli::try_parse_from(["optimist", "project", "list"])?;
/// # Ok::<(), clap::Error>(())
/// ```
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
    server_url: String,
    #[arg(long, global = true, env = "OPTIMIST_PROJECT")]
    project: Option<ProjectId>,
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Table)]
    output: OutputFormat,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Server(ServerArgs),
    Project(ProjectArgs),
    Node(NodeArgs),
    Edge(EdgeArgs),
    Observe(ObserveArgs),
    Estimate(EstimateArgs),
    Scenario(ScenarioArgs),
    Apply(ApplyArgs),
}

/// Executes a parsed command using the appropriate server or HTTP-client path.
///
/// Errors are returned as [`human_errors::Error`] values with recovery advice;
/// binaries should render them once at the process boundary.
///
/// ```no_run
/// use clap::Parser;
/// use optimist::cli::{Cli, run};
///
/// # async fn example() -> Result<(), human_errors::Error> {
/// run(Cli::parse()).await
/// # }
/// ```
pub async fn run(cli: Cli) -> Result<(), human_errors::Error> {
    let server_url = cli.server_url;
    let output = cli.output;
    match cli.command {
        Command::Server(args) => server::run(args).await,
        Command::Project(args) => project::run(args, &server_url, output).await,
        Command::Node(args) => node::run(args, cli.project.as_ref(), &server_url, output).await,
        Command::Edge(args) => edge::run(args, cli.project.as_ref(), &server_url, output).await,
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
