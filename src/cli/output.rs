use clap::ValueEnum;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum OutputFormat {
    Table,
    Json,
    Jsonl,
}
