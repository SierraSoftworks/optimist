use std::process::ExitCode;

use clap::Parser;
use optimist::cli::{Cli, run};

#[tokio::main]
async fn main() -> ExitCode {
    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}", human_errors::pretty(&err));
            ExitCode::FAILURE
        }
    }
}
