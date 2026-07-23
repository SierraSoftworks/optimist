use crate::command::ChangeSetReplay;

use super::{output::OutputFormat, output_json};

pub(super) fn render(
    output: OutputFormat,
    replay: &ChangeSetReplay,
) -> Result<String, human_errors::Error> {
    match output {
        OutputFormat::Table => Ok(table(replay)),
        OutputFormat::Json => output_json::serialize(replay),
        OutputFormat::Jsonl => match &replay.snapshot {
            Some(snapshot) => output_json::lines(&[serde_json::json!({
                "type": "snapshot",
                "value": snapshot,
            })]),
            None => output_json::lines(&replay.changes),
        },
    }
}

fn table(replay: &ChangeSetReplay) -> String {
    if let Some(snapshot) = &replay.snapshot {
        let summary = snapshot.archive.summary();
        return format!(
            "SNAPSHOT_REVISION\tPROJECT\tENTITIES\tEDGES\tSCENARIOS\n{}\t{}\t{}\t{}\t{}",
            snapshot.revision,
            snapshot.archive.project.id,
            summary.entities,
            summary.edges,
            summary.scenarios,
        );
    }
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

#[cfg(test)]
mod tests {
    use crate::{
        command::{ChangeSetReplay, ChangeSnapshot},
        project::{Project, ProjectArchive},
    };

    use super::*;

    fn fallback() -> ChangeSetReplay {
        let project = Project {
            id: crate::domain::ProjectId::new("A").unwrap(),
            name: "Delivery".to_owned(),
            revision: 7,
        };
        ChangeSetReplay {
            after_revision: 0,
            current_revision: 7,
            changes: vec![],
            snapshot: Some(ChangeSnapshot {
                revision: 7,
                archive: ProjectArchive {
                    schema_version: 1,
                    project,
                    description: String::new(),
                    dependence: None,
                    entities: vec![],
                    scenarios: vec![],
                },
            }),
        }
    }

    #[test]
    fn renders_snapshot_replacement_in_table_and_jsonl() {
        let fallback = fallback();
        assert_eq!(
            render(OutputFormat::Table, &fallback).unwrap(),
            "SNAPSHOT_REVISION\tPROJECT\tENTITIES\tEDGES\tSCENARIOS\n7\tA\t0\t0\t0"
        );
        let jsonl = render(OutputFormat::Jsonl, &fallback).unwrap();
        let value: serde_json::Value = serde_json::from_str(&jsonl).unwrap();
        assert_eq!(value["type"], "snapshot");
        assert_eq!(value["value"]["revision"], 7);
    }
}
