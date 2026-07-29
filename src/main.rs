use std::process::ExitCode;

use clap::Parser;
use optimist::cli::{Cli, run};

// A solve allocates a sample set per derived quantity and drops it a pass later,
// tens of thousands of times a second and on every share at once. Measured on
// the shipped examples this is worth about a seventh undivided and better than a
// third once the draws are divided, because the system allocator serialises what
// the shares are doing in parallel.
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

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
