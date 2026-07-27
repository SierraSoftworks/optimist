//! Serving a directory of designs.

use std::{net::SocketAddr, path::PathBuf};

use clap::Args;

use crate::api::{ApiConfig, serve};

/// Options for the server process.
#[derive(Debug, Args)]
pub(super) struct ServerArgs {
    /// Address on which requests are accepted.
    #[arg(long, env = "OPTIMIST_BIND", default_value = "127.0.0.1:3000")]
    bind: SocketAddr,
    /// Directory holding the designs to serve.
    #[arg(long, env = "OPTIMIST_DESIGNS", default_value = "designs")]
    designs: PathBuf,
    /// A frontend build to serve instead of the one this binary would use.
    ///
    /// A release build carries the workbench inside it and a debug build reads
    /// `workbench/dist`; this overrides both, which is what allows a packaged
    /// server to be pointed at a newer frontend without recompiling.
    #[arg(long, env = "OPTIMIST_WEB_ROOT")]
    web_root: Option<PathBuf>,
}

pub(super) async fn run(args: ServerArgs) -> Result<(), human_errors::Error> {
    let designs = args.designs.clone();
    serve(ApiConfig {
        bind: args.bind,
        designs: args.designs,
        web_root: args.web_root,
    })
    .await
    .map_err(|error| {
        human_errors::user(
            format!("The server stopped: {error}"),
            &[
                "Check that the address is free and that the designs directory can be read.",
                "Pass --bind and --designs, or set OPTIMIST_BIND and OPTIMIST_DESIGNS.",
            ],
        )
    })
    .inspect_err(|_| {
        eprintln!("designs were being served from {}", designs.display());
    })
}
