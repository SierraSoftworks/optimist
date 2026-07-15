use std::{net::SocketAddr, path::PathBuf};

use clap::Args;

#[derive(Debug, Args)]
pub struct ServerArgs {
    #[arg(long, default_value = ".optimist")]
    pub data_dir: PathBuf,
    #[arg(long, default_value = "127.0.0.1:3000")]
    pub bind: SocketAddr,
}

pub(super) fn run(_args: ServerArgs) -> Result<(), human_errors::Error> {
    super::unavailable(
        "The Optimist server is not available in this build yet.",
        &[
            "Use `optimist --help` to inspect the command contract while server implementation continues.",
        ],
    )
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
