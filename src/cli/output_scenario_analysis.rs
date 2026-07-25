use crate::domain::{MonteCarloEstimate, ScenarioAnalysis};

use super::{output::OutputFormat, output_json};

pub(super) fn render(
    output: OutputFormat,
    analysis: &ScenarioAnalysis,
) -> Result<String, human_errors::Error> {
    match output {
        OutputFormat::Table => Ok(table(analysis)),
        OutputFormat::Json => output_json::serialize(analysis),
        OutputFormat::Jsonl => analysis
            .candidates
            .iter()
            .flat_map(|candidate| {
                candidate.objectives.iter().map(move |objective| {
                    serde_json::json!({
                        "revision": analysis.revision,
                        "planning_horizon": analysis.planning_horizon,
                        "intervention": candidate.intervention,
                        "objective": objective,
                        "clamped_state_updates": candidate.clamped_state_updates,
                        "feedback_loops": analysis.feedback_loops,
                        "diagnostics": candidate.diagnostics,
                    })
                })
            })
            .map(|row| output_json::serialize(&row))
            .collect::<Result<Vec<_>, _>>()
            .map(|rows| rows.join("\n")),
    }
}

fn table(analysis: &ScenarioAnalysis) -> String {
    let header = "INTERVENTION\tOUTCOME\tREACHABLE\tPERIODS_TO_EFFECT\tDIRECTION\tIMPORTANCE\tBASELINE_MEAN\tFINAL_MEAN\tIMPROVEMENT_MEAN\tIMPROVEMENT_VARIANCE\tCLAMPED_UPDATES\tSAMPLES\tSTATUS";
    std::iter::once(header.to_owned())
        .chain(analysis.candidates.iter().flat_map(|candidate| {
            candidate.objectives.iter().map(|objective| {
                format!(
                    "{}\t{}\t{}\t{}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:?}",
                    candidate.intervention,
                    objective.outcome,
                    objective.reachable,
                    objective
                        .periods_to_effect
                        .map(|periods| periods.to_string())
                        .unwrap_or_else(|| "-".to_owned()),
                    objective.direction,
                    objective.importance,
                    value(&objective.baseline, |estimate| estimate.mean),
                    value(&objective.final_state, |estimate| estimate.mean),
                    value(&objective.improvement, |estimate| estimate.mean),
                    value(&objective.improvement, |estimate| estimate.variance),
                    candidate.clamped_state_updates,
                    candidate.diagnostics.valid_samples,
                    candidate.diagnostics.status,
                )
            })
        }))
        .collect::<Vec<_>>()
        .join("\n")
}

fn value(
    estimate: &MonteCarloEstimate,
    select: impl FnOnce(&MonteCarloEstimate) -> Option<f64>,
) -> String {
    select(estimate)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_owned())
}
