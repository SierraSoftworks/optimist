use std::{net::SocketAddr, path::PathBuf};

use clap::Args;

#[derive(Debug, Args)]
pub(super) struct ServerArgs {
    #[arg(long, default_value = ".optimist")]
    data_dir: PathBuf,
    #[arg(long, default_value = "127.0.0.1:3000")]
    bind: SocketAddr,
    #[arg(long, env = "OPTIMIST_WEB_ROOT")]
    web_root: Option<PathBuf>,
}

pub(super) async fn run(args: ServerArgs) -> Result<(), human_errors::Error> {
    let web_root = args.web_root.or_else(|| {
        let default = PathBuf::from("workbench/dist");
        default.join("index.html").is_file().then_some(default)
    });
    crate::server::serve(crate::server::ServerConfig {
        bind: args.bind,
        data_dir: args.data_dir,
        web_root,
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
    use std::path::PathBuf;

    use clap::Parser;

    use crate::cli::{Cli, Command};

    #[test]
    fn parses_server_mode() {
        let cli = Cli::try_parse_from(["optimist", "server", "--bind", "127.0.0.1:4000"])
            .expect("parse server command");
        assert!(matches!(cli.command, Command::Server(_)));
    }

    #[test]
    fn accepts_an_explicit_workbench_build() {
        let cli = Cli::try_parse_from(["optimist", "server", "--web-root", "workbench/dist"])
            .expect("parse workbench build directory");
        let Command::Server(args) = cli.command else {
            panic!("expected server command")
        };
        assert_eq!(args.web_root, Some(PathBuf::from("workbench/dist")));
    }
}
