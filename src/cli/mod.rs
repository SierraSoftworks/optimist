//! The `optimist` command line.
//!
//! There are two things to do with a design: serve a directory of them so the
//! workbench and other editors can work together, or read one from disk and ask
//! it questions. Everything the tool does is reachable through one of those.

mod output;
mod output_json;
mod server;
mod system;
mod system_output;

use clap::{Parser, Subcommand};

use output::OutputFormat;
use server::ServerArgs;
use system::SystemCommand;

/// Parses the complete `optimist` command line.
///
/// ```
/// use clap::Parser;
/// use optimist::cli::Cli;
///
/// let cli = Cli::try_parse_from(["optimist", "check", "./design"])?;
/// # Ok::<(), clap::Error>(())
/// ```
#[derive(Debug, Parser)]
#[command(
    name = "optimist",
    version,
    about = "Design large systems and find what constrains them"
)]
pub struct Cli {
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Table)]
    output: OutputFormat,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Serves a directory of designs to the workbench.
    Serve(ServerArgs),
    #[command(flatten)]
    Design(SystemCommand),
}

/// Executes a parsed command.
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
    match cli.command {
        Command::Serve(args) => server::run(args).await,
        Command::Design(command) => system::run(command, cli.output),
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Command};

    #[test]
    fn parses_the_commands_over_a_design() {
        for arguments in [
            vec!["optimist", "check", "./design"],
            vec!["optimist", "catalogue", "./design"],
            vec!["optimist", "solve", "./design", "--component", "api"],
            vec![
                "optimist",
                "bottlenecks",
                "./design",
                "--binding",
                "--samples",
                "4000",
            ],
            vec!["optimist", "compare", "./design", "warm-cache"],
        ] {
            let cli = Cli::try_parse_from(&arguments)
                .unwrap_or_else(|error| panic!("{arguments:?}: {error}"));
            assert!(matches!(cli.command, Command::Design(_)));
        }
    }

    #[test]
    fn parses_the_serve_command() {
        let cli =
            Cli::try_parse_from(["optimist", "serve", "--designs", "./designs"]).expect("parses");
        assert!(matches!(cli.command, Command::Serve(_)));
    }

    #[test]
    fn output_format_is_global() {
        let cli = Cli::try_parse_from(["optimist", "--output", "json", "check", "./design"])
            .expect("parses");
        assert!(matches!(cli.command, Command::Design(_)));
    }

    #[test]
    fn a_design_directory_is_required() {
        assert!(Cli::try_parse_from(["optimist", "check"]).is_err());
        assert!(Cli::try_parse_from(["optimist", "compare", "./design"]).is_err());
    }
}
