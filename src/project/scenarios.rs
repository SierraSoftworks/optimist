use crate::{
    command::{CommandOutcome, CreateScenario, DeleteScenario, UpdateScenario},
    domain::{NodeKind, Scenario, ScenarioDraft, ScenarioId},
    store::GraphRepository,
};

use super::{ProjectError, catalog::ProjectEntry};

pub(super) fn create(
    entry: &mut ProjectEntry,
    command: CreateScenario,
) -> Result<CommandOutcome, ProjectError> {
    validate_references(entry, &command.scenario)?;
    let value = entry
        .next_scenario_id
        .ok_or_else(|| ProjectError::ScenarioIdentifierSpaceExhausted(entry.project.id.clone()))?;
    let scenario = Scenario::new(ScenarioId::new(value), command.scenario)?;
    entry.next_scenario_id = value.checked_add(1);
    entry.scenarios.insert(scenario.id, scenario.clone());
    Ok(CommandOutcome::ScenarioCreated(scenario))
}

pub(super) fn update(
    entry: &mut ProjectEntry,
    command: UpdateScenario,
) -> Result<CommandOutcome, ProjectError> {
    command.scenario.validate()?;
    validate_revision(entry, command.id, command.expected_revision)?;
    validate_references(entry, &command.scenario)?;
    let current = entry
        .scenarios
        .get(&command.id)
        .expect("validate_revision found the scenario");
    let revision = current
        .revision
        .checked_add(1)
        .ok_or(ProjectError::ScenarioRevisionSpaceExhausted(command.id))?;
    let scenario = Scenario {
        id: command.id,
        revision,
        draft: command.scenario,
    };
    entry.scenarios.insert(command.id, scenario.clone());
    Ok(CommandOutcome::ScenarioUpdated(scenario))
}

pub(super) fn delete(
    entry: &mut ProjectEntry,
    command: DeleteScenario,
) -> Result<CommandOutcome, ProjectError> {
    validate_revision(entry, command.id, command.expected_revision)?;
    let scenario = entry
        .scenarios
        .remove(&command.id)
        .expect("validate_revision found the scenario");
    Ok(CommandOutcome::ScenarioDeleted(scenario))
}

fn validate_revision(
    entry: &ProjectEntry,
    id: ScenarioId,
    expected: u64,
) -> Result<(), ProjectError> {
    let scenario = entry
        .scenarios
        .get(&id)
        .ok_or(ProjectError::ScenarioNotFound(id))?;
    if scenario.revision != expected {
        return Err(ProjectError::ScenarioRevisionConflict {
            id,
            expected,
            current: scenario.revision,
        });
    }
    Ok(())
}

fn validate_references(
    entry: &mut ProjectEntry,
    scenario: &ScenarioDraft,
) -> Result<(), ProjectError> {
    for objective in &scenario.objectives {
        validate_kind(entry, objective.outcome_id, NodeKind::Outcome)?;
    }
    for candidate in &scenario.candidate_interventions {
        validate_kind(entry, *candidate, NodeKind::Intervention)?;
    }
    Ok(())
}

fn validate_kind(
    entry: &mut ProjectEntry,
    id: crate::domain::EntityId,
    expected: NodeKind,
) -> Result<(), ProjectError> {
    let actual = entry.repository.get_node(id)?.map(|node| node.kind());
    if actual != Some(expected) {
        return Err(ProjectError::InvalidScenarioReference {
            id,
            expected,
            actual,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        command::{
            CommandOutcome, CommandRequest, CreateNode, CreateScenario, DeleteScenario,
            GraphCommand, UpdateScenario,
        },
        domain::{
            EntityId, Intervention, MonteCarloConfig, NodePayload, Outcome, OutcomeDirection,
            ScenarioDraft, ScenarioObjective, UtilityDirection,
        },
    };

    use super::super::{ProjectCatalog, ProjectError};

    fn draft() -> ScenarioDraft {
        ScenarioDraft {
            name: "delivery".to_owned(),
            title: "Delivery".to_owned(),
            rationale: "Choose the next investment.".to_owned(),
            objectives: vec![ScenarioObjective {
                outcome_id: EntityId::new(0),
                direction: UtilityDirection::Maximize,
                importance: 1.0,
            }],
            planning_horizon: 4,
            budgets: vec![],
            candidate_interventions: vec![EntityId::new(1)],
            monte_carlo: MonteCarloConfig::new(7, 10, 100, 0.01, 0.01).unwrap(),
            scalar_preferences: None,
        }
    }

    fn catalog() -> (ProjectCatalog, crate::domain::ProjectId) {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        let outcome = CreateNode {
            name: "reliability".to_owned(),
            title: "Reliability".to_owned(),
            payload: NodePayload::Outcome(Outcome {
                direction: OutcomeDirection::Maximize,
                evidence: vec![],
            }),
        };
        let intervention = CreateNode {
            name: "automation".to_owned(),
            title: "Automation".to_owned(),
            payload: NodePayload::Intervention(Intervention {
                costs: vec![],
                duration: None,
                probability_of_success: None,
                acceptance_criteria: vec![],
            }),
        };
        catalog
            .execute(
                &project.id,
                CommandRequest::new(0, GraphCommand::CreateNode(outcome)),
            )
            .unwrap();
        catalog
            .execute(
                &project.id,
                CommandRequest::new(1, GraphCommand::CreateNode(intervention)),
            )
            .unwrap();
        (catalog, project.id)
    }

    #[test]
    fn stores_independent_scenario_ids_and_replays_retries() {
        let (mut catalog, project) = catalog();
        let request = CommandRequest::new(
            2,
            GraphCommand::CreateScenario(CreateScenario { scenario: draft() }),
        );
        let created = catalog.execute(&project, request.clone()).unwrap();
        assert_eq!(created, catalog.execute(&project, request).unwrap());
        let CommandOutcome::ScenarioCreated(scenario) = created.outcome else {
            panic!("expected scenario")
        };
        assert_eq!(scenario.id.to_string(), "A");
        assert_eq!(catalog.list_scenarios(&project).unwrap(), vec![scenario]);
        assert_eq!(catalog.get(&project).unwrap().revision, 3);
    }

    #[test]
    fn validates_reference_kinds_and_document_revisions() {
        let (mut catalog, project) = catalog();
        let mut invalid = draft();
        invalid.objectives[0].outcome_id = EntityId::new(1);
        let error = catalog.execute(
            &project,
            CommandRequest::new(
                2,
                GraphCommand::CreateScenario(CreateScenario { scenario: invalid }),
            ),
        );
        assert!(matches!(
            error,
            Err(ProjectError::InvalidScenarioReference { .. })
        ));
        assert_eq!(catalog.get(&project).unwrap().revision, 2);

        let created = catalog
            .execute(
                &project,
                CommandRequest::new(
                    2,
                    GraphCommand::CreateScenario(CreateScenario { scenario: draft() }),
                ),
            )
            .unwrap();
        let CommandOutcome::ScenarioCreated(scenario) = created.outcome else {
            unreachable!()
        };
        let updated = catalog
            .execute(
                &project,
                CommandRequest::new(
                    3,
                    GraphCommand::UpdateScenario(UpdateScenario {
                        id: scenario.id,
                        expected_revision: 0,
                        scenario: draft(),
                    }),
                ),
            )
            .unwrap();
        assert!(matches!(
            updated.outcome,
            CommandOutcome::ScenarioUpdated(_)
        ));

        let stale = catalog.execute(
            &project,
            CommandRequest::new(
                4,
                GraphCommand::DeleteScenario(DeleteScenario {
                    id: scenario.id,
                    expected_revision: 0,
                }),
            ),
        );
        assert!(matches!(
            stale,
            Err(ProjectError::ScenarioRevisionConflict { current: 1, .. })
        ));
        assert_eq!(catalog.get(&project).unwrap().revision, 4);
    }
}
