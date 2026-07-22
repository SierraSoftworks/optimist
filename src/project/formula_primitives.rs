use crate::domain::{
    Edge, EdgePayload, Estimate, EstimateAddress, EstimateDimension, EstimateOwner, Formula, Node,
    NodePayload, ProjectId, Unit,
};

use super::FormulaCommandError;

pub(crate) fn from_node(
    project: &ProjectId,
    node: &Node,
) -> Result<Vec<(EstimateAddress, Formula)>, FormulaCommandError> {
    let owner = EstimateOwner::Node(node.id);
    let mut formulas = Vec::new();
    match &node.payload {
        NodePayload::Outcome(value) => states(
            project,
            &owner,
            &value.current,
            &value.desired,
            &mut formulas,
        ),
        NodePayload::Factor(value) => states(
            project,
            &owner,
            &value.current,
            &value.desired,
            &mut formulas,
        ),
        NodePayload::Intervention(value) => {
            for cost in &value.costs {
                push(
                    project,
                    &owner,
                    &cost.value,
                    Unit::base(&cost.dimension).map_err(|_| {
                        FormulaCommandError::InvalidPrimitiveUnit(cost.dimension.clone())
                    })?,
                    &mut formulas,
                );
            }
            optional(
                project,
                &owner,
                &value.duration,
                Unit::base("duration").expect("valid unit"),
                &mut formulas,
            );
            optional(
                project,
                &owner,
                &value.probability_of_success,
                Unit::dimensionless(),
                &mut formulas,
            );
        }
        NodePayload::Metric(value) => optional(
            project,
            &owner,
            &value.current,
            value.quantity.dimension.clone().ok_or_else(|| {
                FormulaCommandError::InvalidPrimitiveUnit(value.quantity.unit.clone())
            })?,
            &mut formulas,
        ),
    }
    Ok(formulas)
}

pub(crate) fn from_edge(project: &ProjectId, edge: &Edge) -> Vec<(EstimateAddress, Formula)> {
    let owner = EstimateOwner::Edge(edge.id());
    let mut formulas = Vec::new();
    match &edge.payload {
        EdgePayload::Contributes(value) | EdgePayload::Changes(value) => {
            if let Some(effect) = value.normalized_effect() {
                push(
                    project,
                    &owner,
                    effect,
                    Unit::dimensionless(),
                    &mut formulas,
                );
            }
            if let Some(response) = value.linear_response() {
                push(
                    project,
                    &owner,
                    &response.destination_change,
                    response.destination_unit.clone(),
                    &mut formulas,
                );
            }
            optional(
                project,
                &owner,
                &value.lag,
                Unit::base("duration").expect("valid unit"),
                &mut formulas,
            );
        }
        EdgePayload::Blocks(value) => {
            push(
                project,
                &owner,
                &value.degree,
                Unit::dimensionless(),
                &mut formulas,
            );
        }
        _ => {}
    }
    formulas
}

fn states(
    project: &ProjectId,
    owner: &EstimateOwner,
    current: &Option<Estimate<crate::domain::NormalizedState>>,
    desired: &Option<Estimate<crate::domain::NormalizedState>>,
    formulas: &mut Vec<(EstimateAddress, Formula)>,
) {
    optional(project, owner, current, Unit::dimensionless(), formulas);
    optional(project, owner, desired, Unit::dimensionless(), formulas);
}

fn optional<T: EstimateDimension>(
    project: &ProjectId,
    owner: &EstimateOwner,
    estimate: &Option<Estimate<T>>,
    unit: Unit,
    formulas: &mut Vec<(EstimateAddress, Formula)>,
) {
    if let Some(estimate) = estimate {
        push(project, owner, estimate, unit, formulas);
    }
}

fn push<T: EstimateDimension>(
    project: &ProjectId,
    owner: &EstimateOwner,
    estimate: &Estimate<T>,
    unit: Unit,
    formulas: &mut Vec<(EstimateAddress, Formula)>,
) {
    formulas.push((
        EstimateAddress::new(project.clone(), owner.clone(), estimate.id),
        Formula::Literal {
            distribution: estimate.distribution.clone(),
            unit,
        },
    ));
}
