//! The `optimist` command line.
//!
//! There are three things to do with a design: open the workbench over a folder
//! of them, serve that folder so the workbench and other editors can work
//! together, or read one from disk and ask it questions. Everything the tool
//! does is reachable through one of those, and running it with nothing to say
//! does the first.
//!
//! The questions come in an order. `check` says whether a design means what its
//! author wrote; `solve` says what flows through it; `bottlenecks` says what it
//! runs out of first; `compare` says whether a proposal helps. Each answers in
//! the boxed, coloured layout this crate already uses for its errors, so a
//! report and a failure look like output from the same program rather than two.
//!
//! `export` and `import` sit beside those because a design that cannot leave the
//! machine it was written on is a design nobody else can review.

mod diagnose;
mod output;
mod output_json;
mod progress;
mod render;
mod report;
mod server;
mod system;
mod transfer;

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

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
    /// Left out to open the workbench in a window.
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Opens the workbench in a window.
    App(AppArgs),
    /// Serves a directory of designs to the workbench.
    Serve(ServerArgs),
    #[command(flatten)]
    Design(SystemCommand),
}

/// Options for the desktop application.
#[derive(Debug, Default, Args)]
struct AppArgs {
    /// Directory holding the designs to open.
    ///
    /// Remembered from the last launch when it is left out, and put under
    /// Documents on the first.
    #[arg(long, env = "OPTIMIST_DESIGNS")]
    designs: Option<PathBuf>,
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
/// # fn example() -> Result<(), human_errors::Error> {
/// run(Cli::parse())
/// # }
/// ```
pub fn run(cli: Cli) -> Result<(), human_errors::Error> {
    cli.colour.apply();
    match cli.command {
        Some(Command::App(args)) => app(args),
        Some(Command::Serve(args)) => runtime()?.block_on(server::run(args)),
        Some(Command::Design(command)) => system::run(command, cli.output, cli.progress),
        None => app(AppArgs::default()),
    }
}

/// Somewhere for the API's timers, solves and handlers to run.
///
/// Built here rather than around `main`, because the desktop application needs
/// the main thread for its event loop and can only be given a runtime that is
/// already running elsewhere.
fn runtime() -> Result<tokio::runtime::Runtime, human_errors::Error> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| {
            human_errors::system(
                format!("The runtime could not be started: {error}"),
                &["Check that this process is allowed to create threads."],
            )
        })
}

#[cfg(feature = "desktop")]
fn app(args: AppArgs) -> Result<(), human_errors::Error> {
    crate::desktop::run(args.designs, runtime()?)
}

/// Says what this build can do instead, rather than doing nothing at all.
#[cfg(not(feature = "desktop"))]
fn app(_args: AppArgs) -> Result<(), human_errors::Error> {
    Err(human_errors::user(
        "This build of optimist has no window to open.".to_owned(),
        &[
            "Run `optimist serve` and open the workbench in a browser.",
            "Run `optimist --help` to see what this build can answer.",
        ],
    ))
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
            assert!(matches!(cli.command, Some(Command::Design(_))));
        }
    }

    #[test]
    fn parses_the_serve_command() {
        let cli =
            Cli::try_parse_from(["optimist", "serve", "--designs", "./designs"]).expect("parses");
        assert!(matches!(cli.command, Some(Command::Serve(_))));
    }

    /// Somebody who double-clicked the application said nothing at all.
    #[test]
    fn nothing_at_all_opens_the_workbench() {
        let cli = Cli::try_parse_from(["optimist"]).expect("parses");
        assert!(cli.command.is_none());
    }

    #[test]
    fn parses_the_app_command() {
        let cli = Cli::try_parse_from(["optimist", "app", "--designs", "./designs"])
            .expect("parses");
        assert!(matches!(cli.command, Some(Command::App(_))));
    }

    #[test]
    fn output_and_colour_are_global() {
        let cli = Cli::try_parse_from([
            "optimist", "--output", "json", "--colour", "never", "check", "./design",
        ])
        .expect("parses");
        assert!(matches!(cli.command, Some(Command::Design(_))));
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
