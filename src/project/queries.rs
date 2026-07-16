use crate::{
    domain::{
        Edge, EdgeId, EntityId, EstimateAddress, FormulaCatalog, FormulaDefinition, Node,
        PrimitiveEstimate, ProjectDependenceModel, ProjectId, Scenario, ScenarioId,
    },
    store::GraphRepository,
};

use super::{ProjectCatalog, ProjectError};

impl ProjectCatalog {
    /// Lists complete node aggregates for one project in deterministic ID order.
    pub fn list_nodes(&mut self, project: &ProjectId) -> Result<Vec<Node>, ProjectError> {
        Ok(self.repository_mut(project)?.list_nodes()?)
    }

    /// Retrieves one complete node aggregate from a project-local entity ID.
    pub fn get_node(
        &mut self,
        project: &ProjectId,
        entity: EntityId,
    ) -> Result<Option<Node>, ProjectError> {
        Ok(self.repository_mut(project)?.get_node(entity)?)
    }

    /// Lists complete edge aggregates in canonical edge-ID order.
    pub fn list_edges(&mut self, project: &ProjectId) -> Result<Vec<Edge>, ProjectError> {
        Ok(self.repository_mut(project)?.list_edges()?)
    }

    /// Retrieves one complete edge aggregate by canonical identity.
    pub fn get_edge(
        &mut self,
        project: &ProjectId,
        edge: &EdgeId,
    ) -> Result<Option<Edge>, ProjectError> {
        Ok(self.repository_mut(project)?.get_edge(edge)?)
    }

    /// Retrieves one primitive estimate by stable project/owner-local address.
    pub fn get_estimate(
        &mut self,
        project: &ProjectId,
        address: &EstimateAddress,
    ) -> Result<PrimitiveEstimate, ProjectError> {
        let entry = self
            .projects
            .get_mut(project)
            .ok_or_else(|| ProjectError::NotFound(project.clone()))?;
        super::estimate::get(entry, address)
    }

    /// Lists compiled formulas and the current formula document revision.
    pub fn list_formulas(&mut self, project: &ProjectId) -> Result<FormulaCatalog, ProjectError> {
        let entry = self
            .projects
            .get_mut(project)
            .ok_or_else(|| ProjectError::NotFound(project.clone()))?;
        super::formulas::list(entry)
    }

    /// Retrieves one compiled Fermi component by nested estimate address.
    pub fn get_formula(
        &mut self,
        project: &ProjectId,
        address: &EstimateAddress,
    ) -> Result<FormulaDefinition, ProjectError> {
        let entry = self
            .projects
            .get_mut(project)
            .ok_or_else(|| ProjectError::NotFound(project.clone()))?;
        super::formulas::show(entry, address)
    }

    /// Lists scenario documents in deterministic project-local ID order.
    pub fn list_scenarios(&self, project: &ProjectId) -> Result<Vec<Scenario>, ProjectError> {
        let entry = self
            .projects
            .get(project)
            .ok_or_else(|| ProjectError::NotFound(project.clone()))?;
        Ok(entry.scenarios.values().cloned().collect())
    }

    /// Retrieves one scenario document without exposing graph storage internals.
    pub fn get_scenario(
        &self,
        project: &ProjectId,
        scenario: ScenarioId,
    ) -> Result<Option<Scenario>, ProjectError> {
        let entry = self
            .projects
            .get(project)
            .ok_or_else(|| ProjectError::NotFound(project.clone()))?;
        Ok(entry.scenarios.get(&scenario).cloned())
    }

    /// Retrieves the singleton project dependence document.
    pub fn get_dependence(
        &self,
        project: &ProjectId,
    ) -> Result<Option<ProjectDependenceModel>, ProjectError> {
        let entry = self
            .projects
            .get(project)
            .ok_or_else(|| ProjectError::NotFound(project.clone()))?;
        Ok(entry.dependence.clone())
    }
}
