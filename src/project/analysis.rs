use crate::{
    domain::{
        AnalysisLimits, AnalysisRevisionKey, ImpedimentAnalysis, ScenarioAnalysis, ScenarioId,
        StateDetail, StructuralAnalysis,
    },
    store::GraphRepository,
};

use super::{ProjectCatalog, ProjectError};

impl ProjectCatalog {
    /// Projects factor-to-outcome impediment review candidates from one snapshot.
    ///
    /// Topology and evidence-priority order remain separate in the result. This
    /// method does not infer causal effects from evidence or combine evidence into
    /// a scalar confidence score.
    pub fn analyze_impediments(
        &self,
        project_id: &crate::domain::ProjectId,
    ) -> Result<ImpedimentAnalysis, ProjectError> {
        let entry = self
            .projects
            .get(project_id)
            .ok_or_else(|| ProjectError::NotFound(project_id.clone()))?;
        let revision = AnalysisRevisionKey {
            project: project_id.clone(),
            graph_revision: entry.graph_revision,
            scenario: None,
            dependence_revision: entry.dependence.as_ref().map(|model| model.revision),
        };
        let nodes = entry.repository.list_nodes()?;
        let edges = entry.repository.list_edges()?;
        Ok(ImpedimentAnalysis::compute(revision, &nodes, &edges)?)
    }

    /// Computes exact structural topology from one immutable project snapshot.
    ///
    /// The revision key includes the independently tracked graph revision plus the
    /// selected scenario and current dependence document revision. The
    /// current implementation computes eagerly and returns no cached stale result.
    pub fn analyze_structure(
        &self,
        project_id: &crate::domain::ProjectId,
        scenario_id: Option<ScenarioId>,
        limits: AnalysisLimits,
    ) -> Result<StructuralAnalysis, ProjectError> {
        let entry = self
            .projects
            .get(project_id)
            .ok_or_else(|| ProjectError::NotFound(project_id.clone()))?;
        let scenario = match scenario_id {
            Some(id) => Some((
                id,
                entry
                    .scenarios
                    .get(&id)
                    .ok_or(ProjectError::ScenarioNotFound(id))?
                    .revision,
            )),
            None => None,
        };
        let revision = AnalysisRevisionKey {
            project: project_id.clone(),
            graph_revision: entry.graph_revision,
            scenario,
            dependence_revision: entry.dependence.as_ref().map(|model| model.revision),
        };
        let nodes = entry.repository.list_nodes()?;
        let edges = entry.repository.list_edges()?;
        Ok(StructuralAnalysis::compute(
            revision, &nodes, &edges, limits,
        )?)
    }

    /// Projects each scenario candidate over its finite planning horizon.
    ///
    /// The returned key captures every independently revisioned input document,
    /// including the residual dependence document whose copulas couple the
    /// estimates it names. `detail` decides whether every propagated state keeps
    /// its path, which is what an author reads when a projection ends somewhere
    /// surprising and the objectives alone do not say why.
    pub fn analyze_scenario(
        &self,
        project_id: &crate::domain::ProjectId,
        scenario_id: ScenarioId,
        detail: StateDetail,
    ) -> Result<ScenarioAnalysis, ProjectError> {
        let entry = self
            .projects
            .get(project_id)
            .ok_or_else(|| ProjectError::NotFound(project_id.clone()))?;
        let scenario = entry
            .scenarios
            .get(&scenario_id)
            .ok_or(ProjectError::ScenarioNotFound(scenario_id))?
            .clone();
        let revision = AnalysisRevisionKey {
            project: project_id.clone(),
            graph_revision: entry.graph_revision,
            scenario: Some((scenario.id, scenario.revision)),
            dependence_revision: entry.dependence.as_ref().map(|model| model.revision),
        };
        let nodes = entry.repository.list_nodes()?;
        let edges = entry.repository.list_edges()?;
        Ok(ScenarioAnalysis::compute(
            revision,
            &scenario,
            &nodes,
            &edges,
            entry.dependence.as_ref(),
            detail,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        command::{CommandRequest, CreateNode, CreateScenario, GraphCommand},
        domain::{
            AnalysisLimits, Factor, MonteCarloConfig, NodePayload, ScenarioDraft, ScenarioId,
            StateDetail,
        },
        project::{ProjectCatalog, ProjectError},
    };

    #[test]
    fn keys_graph_and_scenario_revisions_independently() {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        let initial = catalog
            .analyze_structure(&project.id, None, AnalysisLimits::default())
            .unwrap();
        assert_eq!(initial.revision.graph_revision, 0);
        assert_eq!(initial.revision.scenario, None);

        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    0,
                    GraphCommand::CreateNode(CreateNode {
                        name: "flow".to_owned(),
                        title: "Flow".to_owned(),
                        payload: NodePayload::Factor(Factor {
                            controllable: true,
                            evidence: vec![],
                        }),
                    }),
                ),
            )
            .unwrap();
        let graph = catalog
            .analyze_structure(&project.id, None, AnalysisLimits::default())
            .unwrap();
        assert_eq!(graph.revision.graph_revision, 1);

        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    1,
                    GraphCommand::CreateScenario(CreateScenario {
                        scenario: ScenarioDraft {
                            name: "plan".to_owned(),
                            title: "Plan".to_owned(),
                            rationale: String::new(),
                            objectives: vec![],
                            planning_horizon: 4,
                            budgets: vec![],
                            candidate_interventions: vec![],
                            monte_carlo: MonteCarloConfig::new(1, 2, 10, 0.1, 0.1).unwrap(),
                            scalar_preferences: None,
                        },
                    }),
                ),
            )
            .unwrap();
        let scenario = catalog
            .analyze_structure(
                &project.id,
                Some(ScenarioId::new(0)),
                AnalysisLimits::default(),
            )
            .unwrap();
        assert_eq!(scenario.revision.graph_revision, 1);
        assert_eq!(scenario.revision.scenario, Some((ScenarioId::new(0), 0)));
        let projection = catalog
            .analyze_scenario(&project.id, ScenarioId::new(0), StateDetail::Omitted)
            .unwrap();
        assert_eq!(projection.revision, scenario.revision);
        assert_eq!(projection.planning_horizon, 4);
        assert!(projection.candidates.is_empty());
        assert!(matches!(
            catalog.analyze_structure(
                &project.id,
                Some(ScenarioId::new(1)),
                AnalysisLimits::default(),
            ),
            Err(ProjectError::ScenarioNotFound(_))
        ));
    }
}
