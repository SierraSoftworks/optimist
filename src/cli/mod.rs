//! The `optimist` command line.
//!
//! There are two things to do with a design: serve a directory of them so the
//! workbench and other editors can work together, or read one from disk and ask
//! it questions. Everything the tool does is reachable through one of those.
//!
//! The questions come in an order. `check` says whether a design means what its
//! author wrote; `solve` says what flows through it; `bottlenecks` says what it
//! runs out of first; `compare` says whether a proposal helps. Each answers in
//! the boxed, coloured layout this crate already uses for its errors, so a
//! report and a failure look like output from the same program rather than two.

mod diagnose;
mod output;
mod output_json;
mod progress;
mod render;
mod report;
mod server;
mod system;

use clap::{Parser, Subcommand};

use output::{ColourChoice, OutputFormat};
use progress::ProgressChoice;
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
    /// Report format.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Table)]
    output: OutputFormat,
    /// When to colour the output.
    #[arg(long, global = true, value_enum, default_value_t = ColourChoice::Auto)]
    colour: ColourChoice,
    /// When to draw a progress bar on standard error while solving.
    #[arg(long, global = true, value_enum, default_value_t = ProgressChoice::Auto)]
    progress: ProgressChoice,
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
    cli.colour.apply();
    match cli.command {
        Command::Serve(args) => server::run(args).await,
        Command::Design(command) => system::run(command, cli.output, cli.progress),
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
            vec!["optimist", "check", "--no-solve"],
            vec!["optimist", "catalogue", "./design", "--type", "compute"],
            vec!["optimist", "solve", "./design", "--component", "api"],
            vec!["optimist", "solve", "-c", "api", "-i", "warm-cache"],
            vec![
                "optimist",
                "bottlenecks",
                "./design",
                "--binding",
                "--samples",
                "4000",
            ],
            vec!["optimist", "compare", "./design", "warm-cache", "shard"],
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
    fn output_and_colour_are_global() {
        let cli = Cli::try_parse_from([
            "optimist", "--output", "json", "--colour", "never", "check", "./design",
        ])
        .expect("parses");
        assert!(matches!(cli.command, Command::Design(_)));
    }

    #[test]
    fn the_design_directory_defaults_to_the_working_one() {
        assert!(Cli::try_parse_from(["optimist", "check"]).is_ok());
        assert!(Cli::try_parse_from(["optimist", "bottlenecks"]).is_ok());
    }

    #[test]
    fn comparing_requires_something_to_compare() {
        assert!(Cli::try_parse_from(["optimist", "compare", "./design"]).is_err());
    }
}
