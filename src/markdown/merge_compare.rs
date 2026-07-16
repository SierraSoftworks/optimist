use super::{EntityDocument, ProjectDocument, ScenarioDocument};

pub(super) fn project(left: &ProjectDocument, right: &ProjectDocument) -> bool {
    left.schema_version == right.schema_version
        && left.project.id == right.project.id
        && left.project.name == right.project.name
        && left.dependence == right.dependence
        && left.formulas == right.formulas
        && left.description == right.description
}

pub(super) fn entity(left: &EntityDocument, right: &EntityDocument) -> bool {
    left.schema_version == right.schema_version
        && left.node == right.node
        && left.outgoing_edges == right.outgoing_edges
}

pub(super) fn scenario(left: &ScenarioDocument, right: &ScenarioDocument) -> bool {
    left.schema_version == right.schema_version && left.scenario == right.scenario
}
