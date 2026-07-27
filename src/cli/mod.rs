mod analysis;
mod analysis_client;
mod apply;
mod batch;
mod batch_client;
mod client;
mod client_advice;
mod dependence;
mod dependence_client;
mod edge;
mod edge_client;
mod edge_payload;
mod estimate;
mod estimate_client;
mod node;
mod node_client;
mod node_payload;
mod node_update_client;
mod observe;
mod observe_client;
mod output;
mod output_backup;
mod output_batch;
mod output_json;
mod output_scenario_analysis;
mod output_table;
mod output_table_backup;
mod project;
mod project_archive_client;
mod project_backup;
mod project_backup_client;
mod project_changes_client;
mod project_changes_output;
mod scenario;
mod scenario_client;
mod server;
mod system;
mod system_output;

use clap::{Parser, Subcommand};

use analysis::AnalysisArgs;
use apply::ApplyArgs;
use batch::BatchArgs;
use dependence::DependenceArgs;
use edge::EdgeArgs;
use estimate::EstimateArgs;
use node::NodeArgs;
use observe::ObserveArgs;
use output::OutputFormat;
use project::ProjectArgs;
use scenario::ScenarioArgs;
use server::ServerArgs;
use system::SystemArgs;

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
    Dependence(DependenceArgs),
    Apply(ApplyArgs),
    Batch(BatchArgs),
    Analysis(AnalysisArgs),
    /// Read, solve, and compare a system design held in a directory.
    System(SystemArgs),
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
        Command::Observe(args) => {
            observe::run(args, cli.project.as_ref(), &server_url, output).await
        }
        Command::Estimate(args) => {
            estimate::run(args, cli.project.as_ref(), &server_url, output).await
        }
        Command::Scenario(args) => {
            scenario::run(args, cli.project.as_ref(), &server_url, output).await
        }
        Command::Dependence(args) => {
            dependence::run(args, cli.project.as_ref(), &server_url, output).await
        }
        Command::Apply(args) => apply::run(args),
        Command::Batch(args) => batch::run(args, cli.project.as_ref(), &server_url, output).await,
        Command::Analysis(args) => {
            analysis::run(args, cli.project.as_ref(), &server_url, output).await
        }
        Command::System(args) => system::run(args, output),
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

    #[test]
    fn parses_system_design_commands() {
        for arguments in [
            vec!["optimist", "system", "check", "./design"],
            vec!["optimist", "system", "catalogue", "./design"],
            vec![
                "optimist",
                "system",
                "solve",
                "./design",
                "--component",
                "api",
            ],
            vec![
                "optimist",
                "system",
                "bottlenecks",
                "./design",
                "--binding",
                "--samples",
                "4000",
            ],
            vec!["optimist", "system", "compare", "./design", "warm-cache"],
        ] {
            let cli = Cli::try_parse_from(&arguments)
                .unwrap_or_else(|error| panic!("{arguments:?}: {error}"));
            assert!(matches!(cli.command, Command::System(_)));
        }
    }

    #[test]
    fn system_commands_require_a_design_directory() {
        assert!(Cli::try_parse_from(["optimist", "system", "check"]).is_err());
        assert!(Cli::try_parse_from(["optimist", "system", "compare", "./design"]).is_err());
    }
}
