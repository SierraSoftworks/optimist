use std::{net::SocketAddr, path::PathBuf};

use clap::Args;

#[derive(Debug, Args)]
pub(super) struct ServerArgs {
    #[arg(long, default_value = ".optimist")]
    data_dir: PathBuf,
    #[arg(long, default_value = "127.0.0.1:3000")]
    bind: SocketAddr,
}

pub(super) async fn run(args: ServerArgs) -> Result<(), human_errors::Error> {
    crate::server::serve(crate::server::ServerConfig {
        bind: args.bind,
        data_dir: args.data_dir,
    })
    .await
    .map_err(|error| {
        human_errors::wrap_system(
            error,
            "The Optimist server stopped unexpectedly.",
            &[
                "Check that the bind address is available and the data directory is writable.",
                "Retry with `optimist server --bind 127.0.0.1:3001` if another process uses the port.",
            ],
        )
    })
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cli::{Cli, Command};

    #[test]
    fn parses_server_mode() {
        let cli = Cli::try_parse_from(["optimist", "server", "--bind", "127.0.0.1:4000"])
            .expect("parse server command");
        assert!(matches!(cli.command, Command::Server(_)));
    }
}
