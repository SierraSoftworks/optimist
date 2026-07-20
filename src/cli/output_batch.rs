use crate::command::CommandBatchResult;

use super::{output::OutputFormat, output_json};

impl OutputFormat {
    pub(super) fn command_batch(
        self,
        result: &CommandBatchResult,
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(format!(
                "BATCH\tBASE_REVISION\tPROJECT_REVISION\tCOMPENSATES\tCOMMANDS\n{}\t{}\t{}\t{}\t{}",
                result.request_id,
                result.base_revision,
                result.project_revision,
                result
                    .compensates
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "-".to_owned()),
                result.results.len()
            )),
            Self::Json | Self::Jsonl => output_json::serialize(result),
        }
    }
}
