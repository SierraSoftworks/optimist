use crate::{
    command::{
        CommandOutcome, CommandRequest, CommandResult, GraphCommand, RemoveFormula, SetFormula,
    },
    domain::{EstimateAddress, Formula, FormulaCatalog, FormulaDefinition, ProjectId},
};

use super::client::{ProjectClient, decode};

impl ProjectClient {
    pub(super) async fn set_formula(
        &self,
        project: &ProjectId,
        address: EstimateAddress,
        formula: Formula,
        provenance: Vec<String>,
    ) -> Result<FormulaDefinition, human_errors::Error> {
        let expected_revision = self.list_formulas(project).await?.revision;
        self.formula_command(
            project,
            GraphCommand::SetFormula(SetFormula {
                address,
                formula,
                expected_revision,
                provenance,
            }),
        )
        .await
    }

    pub(super) async fn remove_formula(
        &self,
        project: &ProjectId,
        address: EstimateAddress,
    ) -> Result<FormulaDefinition, human_errors::Error> {
        let expected_revision = self.list_formulas(project).await?.revision;
        self.formula_command(
            project,
            GraphCommand::RemoveFormula(RemoveFormula {
                address,
                expected_revision,
            }),
        )
        .await
    }

    pub(super) async fn list_formulas(
        &self,
        project: &ProjectId,
    ) -> Result<FormulaCatalog, human_errors::Error> {
        let response = self
            .client
            .get(self.endpoint(&format!("api/v1/projects/{project}/formulas"))?)
            .send()
            .await
            .map_err(network_error)?;
        decode(response).await
    }

    pub(super) async fn show_formula(
        &self,
        project: &ProjectId,
        address: &EstimateAddress,
    ) -> Result<FormulaDefinition, human_errors::Error> {
        let response = self
            .client
            .get(self.endpoint(&format!("api/v1/projects/{project}/formula"))?)
            .query(&[("address", address.to_string())])
            .send()
            .await
            .map_err(network_error)?;
        decode(response).await
    }

    async fn formula_command(
        &self,
        project: &ProjectId,
        command: GraphCommand,
    ) -> Result<FormulaDefinition, human_errors::Error> {
        let project_revision = self.show(project).await?.revision;
        let response = self
            .client
            .post(self.endpoint(&format!("api/v1/projects/{project}/commands"))?)
            .json(&CommandRequest::new(project_revision, command))
            .send()
            .await
            .map_err(network_error)?;
        let result: CommandResult = decode(response).await?;
        match result.outcome {
            CommandOutcome::FormulaSet(value) | CommandOutcome::FormulaRemoved(value) => Ok(value),
            _ => Err(human_errors::system(
                "The Optimist server returned an unexpected result for a formula command.",
                &["Confirm the CLI and server versions match, then inspect the server logs."],
            )),
        }
    }
}

fn network_error(error: reqwest::Error) -> human_errors::Error {
    human_errors::wrap_system(
        error,
        "The Optimist server could not be reached.",
        &["Start `optimist server` and verify `--server-url` or `OPTIMIST_SERVER` points to it."],
    )
}

#[cfg(test)]
mod tests {
    use tokio::{net::TcpListener, task::JoinHandle};

    use crate::{
        domain::{
            EntityId, EstimateAddress, EstimateComponentId, EstimateId, EstimateOwner,
            EstimateSlot, Factor, Formula, NodePayload, QuantityDefinition, QuantitySupport, Unit,
        },
        server,
    };

    use super::ProjectClient;

    async fn client() -> (ProjectClient, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, server::router()).await.unwrap();
        });
        (
            ProjectClient::new(&format!("http://{address}")).unwrap(),
            task,
        )
    }

    #[tokio::test]
    async fn performs_formula_lifecycle_over_http() {
        let (client, server) = client().await;
        let project = client.create("Delivery".to_owned()).await.unwrap();
        client
            .create_node(
                &project.id,
                "flow".to_owned(),
                "Flow".to_owned(),
                NodePayload::Factor(Factor {
                    controllable: true,
                    evidence: vec![],
                }),
            )
            .await
            .unwrap();
        client
            .set_node_quantity_state(
                &project.id,
                EntityId::new(0),
                QuantityDefinition::with_dimension(
                    "state",
                    Some(Unit::dimensionless()),
                    None,
                    QuantitySupport::Bounded {
                        lower: 0.0,
                        upper: 1.0,
                    },
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let root = EstimateAddress::new(
            project.id.clone(),
            EstimateOwner::Node(EntityId::new(0)),
            EstimateId::new(0),
        );
        client
            .set_squiggle_estimate(
                &project.id,
                root.clone(),
                EstimateSlot::Current,
                crate::domain::SquiggleEstimateDefinition {
                    source: "beta(2, 3)".to_owned(),
                    seed: 42,
                    sample_count: 256,
                    target_unit: crate::domain::Unit::dimensionless(),
                },
                vec![],
                crate::domain::EstimateUncertainty::default(),
            )
            .await
            .unwrap();
        let component = root
            .clone()
            .with_component(EstimateComponentId::new("baseline").unwrap());
        let created = client
            .set_formula(
                &project.id,
                component.clone(),
                Formula::Reference { address: root },
                vec!["decomposition".to_owned()],
            )
            .await
            .unwrap();
        let listed = client.list_formulas(&project.id).await.unwrap();
        assert_eq!(listed.revision, 1);
        assert_eq!(listed.formulas, vec![created.clone()]);
        assert_eq!(
            client.show_formula(&project.id, &component).await.unwrap(),
            created
        );
        let in_use = client
            .remove_estimate(&project.id, created.compiled.dependencies[0].clone())
            .await
            .unwrap_err();
        assert!(in_use.advice().iter().any(|item| item.contains("formula")));
        assert_eq!(
            client
                .remove_formula(&project.id, component.clone())
                .await
                .unwrap(),
            created
        );
        let error = client
            .show_formula(&project.id, &component)
            .await
            .unwrap_err();
        assert!(
            error
                .advice()
                .iter()
                .any(|item| item.contains("formula list"))
        );
        server.abort();
    }
}
