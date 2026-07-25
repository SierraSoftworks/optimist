use std::collections::BTreeMap;

use super::{
    Edge, EdgePayload, EntityId, Node, NodePayload, QuantityDefinition, RelationBindings,
    RelationError, RelationProgram, RelationSchema, StateRelation, StateRelationError,
};

/// Derives the names one node's equation may reference from the graph around it.
///
/// The schema is never authored. Parents are the states that already contribute
/// to this one, activations are the interventions that already change it, and
/// the result unit is the node's own. A relation therefore cannot invent a
/// dependency the graph does not show, which keeps the drawn model and the
/// arithmetic that runs it from diverging.
pub(super) fn schema(
    node: &Node,
    nodes: &BTreeMap<EntityId, &Node>,
    edges: &[Edge],
    relation: &StateRelation,
) -> Result<RelationSchema, StateRelationError> {
    let result_unit = quantity(node)
        .ok_or(RelationError::UnknownResultUnit)?
        .estimate_target()?
        .1;
    let mut schema = RelationSchema::new(result_unit);
    for edge in edges.iter().filter(|edge| edge.destination == node.id) {
        let Some(source) = nodes.get(&edge.source) else {
            continue;
        };
        match edge.payload {
            EdgePayload::Contributes(_) | EdgePayload::Blocks(_) => {
                let unit = quantity(source)
                    .ok_or_else(|| RelationError::UnmeasuredParent(source.name.clone()))?
                    .estimate_target()?
                    .1;
                schema.parents.insert(source.name.clone(), unit);
            }
            EdgePayload::Changes(_) => {
                schema.activations.insert(source.name.clone());
            }
            _ => {}
        }
    }
    for (name, parameter) in &relation.parameters {
        schema
            .parameters
            .insert(name.clone(), parameter.quantity.estimate_target()?.1);
    }
    Ok(schema)
}

/// Compiles a relation against the graph, reporting where the arithmetic fails.
pub(crate) fn compile(
    node: &Node,
    nodes: &BTreeMap<EntityId, &Node>,
    edges: &[Edge],
    relation: &StateRelation,
) -> Result<RelationProgram, StateRelationError> {
    let schema = schema(node, nodes, edges, relation)?;
    let program = RelationProgram::compile(&relation.source, &schema)?;
    reject_non_numeric(&program, &schema)?;
    Ok(program)
}

/// Rejects an equation whose result is a distribution rather than a number.
///
/// The linter accepts a distribution wherever a number is allowed, so this is
/// caught by evaluating once with every binding set to one. That proves only the
/// shape of the result, which is the point: other failures are ignored because
/// placeholder inputs can hit a domain error that real values never would.
fn reject_non_numeric(
    program: &RelationProgram,
    schema: &RelationSchema,
) -> Result<(), StateRelationError> {
    let mut runtime = RelationProgram::runtime(0)?;
    let bindings = RelationBindings {
        baseline: 1.0,
        parents: schema
            .parents
            .keys()
            .map(|name| (name.clone(), 1.0))
            .collect(),
        parameters: schema
            .parameters
            .keys()
            .map(|name| (name.clone(), 1.0))
            .collect(),
        activations: schema
            .activations
            .iter()
            .map(|name| (name.clone(), 1.0))
            .collect(),
    };
    match program.evaluate(&mut runtime, &bindings) {
        Err(error @ RelationError::NonNumericResult) => Err(error.into()),
        _ => Ok(()),
    }
}

/// Returns the native quantity a node's value is measured against.
pub(super) fn quantity(node: &Node) -> Option<&QuantityDefinition> {
    match &node.payload {
        NodePayload::Metric(metric) => Some(&metric.quantity),
        _ => node.native_state.as_ref().map(|state| &state.quantity),
    }
}

/// Returns the equation computing a node's value, if it has one.
pub(crate) fn relation_of(node: &Node) -> Option<&StateRelation> {
    match &node.payload {
        NodePayload::Metric(metric) => metric.relation.as_ref(),
        _ => node
            .native_state
            .as_ref()
            .and_then(|state| state.relation.as_ref()),
    }
}
