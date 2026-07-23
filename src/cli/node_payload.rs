use crate::domain::{Factor, Intervention, Metric, NodePayload, Outcome, OutcomeDirection};

use super::node::{Direction, NodeType};

pub(super) fn build(
    kind: NodeType,
    direction: Option<Direction>,
    unit: Option<String>,
    aggregation: Option<String>,
    controllable: bool,
) -> Result<NodePayload, human_errors::Error> {
    match kind {
        NodeType::Outcome if unit.is_none() && aggregation.is_none() && !controllable => {
            let direction =
                direction.ok_or_else(|| invalid("Outcome nodes require `--direction`."))?;
            Ok(NodePayload::Outcome(Outcome {
                direction: direction.into(),
                evidence: vec![],
            }))
        }
        NodeType::Metric if direction.is_none() && !controllable => {
            let unit = unit.ok_or_else(|| invalid("Metric nodes require `--unit`."))?;
            Metric::new(unit, aggregation)
                .map(NodePayload::Metric)
                .map_err(|_| invalid("Metric nodes require a nonempty valid `--unit`."))
        }
        NodeType::Factor if direction.is_none() && unit.is_none() && aggregation.is_none() => {
            Ok(NodePayload::Factor(Factor {
                controllable,
                evidence: vec![],
            }))
        }
        NodeType::Intervention
            if direction.is_none() && unit.is_none() && aggregation.is_none() && !controllable =>
        {
            Ok(NodePayload::Intervention(Intervention {
                costs: vec![],
                duration: None,
                probability_of_success: None,
                acceptance_criteria: vec![],
            }))
        }
        _ => Err(invalid(
            "The supplied options do not apply to the selected node kind.",
        )),
    }
}

fn invalid(message: &'static str) -> human_errors::Error {
    human_errors::user(
        message,
        &[
            "Run `optimist node create --help` and provide only fields belonging to the selected kind.",
        ],
    )
}

impl From<Direction> for OutcomeDirection {
    fn from(value: Direction) -> Self {
        match value {
            Direction::Maximize => Self::Maximize,
            Direction::Minimize => Self::Minimize,
            Direction::TargetRange => Self::TargetRange,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::NodePayload;

    use super::{Direction, NodeType, build};

    #[test]
    fn requires_explicit_outcome_direction() {
        assert!(build(NodeType::Outcome, None, None, None, false).is_err());
        assert!(matches!(
            build(
                NodeType::Outcome,
                Some(Direction::Maximize),
                None,
                None,
                false
            ),
            Ok(NodePayload::Outcome(_))
        ));
    }

    #[test]
    fn rejects_options_from_another_kind() {
        assert!(
            build(
                NodeType::Factor,
                Some(Direction::Minimize),
                None,
                None,
                false
            )
            .is_err()
        );
    }
}
