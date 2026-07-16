use crate::command::ChangeSetReplay;

use super::{output::OutputFormat, output_json};

pub(super) fn render(
    output: OutputFormat,
    replay: &ChangeSetReplay,
) -> Result<String, human_errors::Error> {
    match output {
        OutputFormat::Table => Ok(table(replay)),
        OutputFormat::Json => output_json::serialize(replay),
        OutputFormat::Jsonl => output_json::lines(&replay.changes),
    }
}

fn table(replay: &ChangeSetReplay) -> String {
    let rows = replay.changes.iter().map(|change| {
        let command = serde_json::to_value(&change.command)
            .ok()
            .and_then(|value| {
                value
                    .get("type")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "unknown".to_owned());
        let outcome = serde_json::to_value(&change.outcome)
            .ok()
            .and_then(|value| {
                value
                    .get("type")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "unknown".to_owned());
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            change.project_revision,
            change.base_revision,
            change.graph_revision,
            change.request_id,
            command,
            outcome
        )
    });
    std::iter::once(
        "PROJECT_REVISION\tBASE_REVISION\tGRAPH_REVISION\tREQUEST_ID\tCOMMAND\tOUTCOME".to_owned(),
    )
    .chain(rows)
    .collect::<Vec<_>>()
    .join("\n")
}
