use crate::domain::{AnalysisLimits, ProjectId, ScenarioId, StructuralAnalysis};

use super::client::{ProjectClient, decode};

impl ProjectClient {
    pub(super) async fn analyze_structure(
        &self,
        project: &ProjectId,
        scenario: Option<ScenarioId>,
        limits: AnalysisLimits,
    ) -> Result<StructuralAnalysis, human_errors::Error> {
        let mut request = self
            .client
            .get(self.endpoint(&format!("api/v1/projects/{project}/analysis/structure"))?)
            .query(&[
                (
                    "maximum_cycle_length",
                    limits.maximum_cycle_length.to_string(),
                ),
                ("maximum_cycles", limits.maximum_cycles.to_string()),
            ]);
        if let Some(scenario) = scenario {
            request = request.query(&[("scenario", scenario.to_string())]);
        }
        let response = request.send().await.map_err(network_error)?;
        decode(response).await
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
            AnalysisLimits, BlockingEffect, Distribution, EdgePayload, Estimate, EstimateId,
            Factor, NodePayload, SignedInfluence,
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

    fn block() -> EdgePayload {
        EdgePayload::Blocks(BlockingEffect {
            degree: Estimate::<SignedInfluence>::new(
                EstimateId::new(0),
                Distribution::scaled_beta(2.0, 2.0, -1.0, 1.0).unwrap(),
            )
            .unwrap(),
        })
    }

    #[tokio::test]
    async fn computes_exact_structural_analysis_over_http() {
        let (client, server) = client().await;
        let project = client.create("Delivery".to_owned()).await.unwrap();
        for name in ["left", "right"] {
            client
                .create_node(
                    &project.id,
                    name.to_owned(),
                    name.to_owned(),
                    NodePayload::Factor(Factor {
                        current: None,
                        desired: None,
                        controllable: false,
                        evidence: vec![],
                    }),
                )
                .await
                .unwrap();
        }
        let nodes = client.list_nodes(&project.id).await.unwrap();
        client
            .create_edge(&project.id, nodes[0].id, nodes[1].id, block())
            .await
            .unwrap();
        client
            .create_edge(&project.id, nodes[1].id, nodes[0].id, block())
            .await
            .unwrap();
        let analysis = client
            .analyze_structure(&project.id, None, AnalysisLimits::new(4, 10).unwrap())
            .await
            .unwrap();
        assert_eq!(analysis.revision.graph_revision, 4);
        assert_eq!(analysis.components.len(), 1);
        assert!(analysis.components[0].is_feedback);
        assert_eq!(analysis.cycles.len(), 1);
        assert!(!analysis.cycles_truncated);

        let error = client
            .analyze_structure(
                &project.id,
                None,
                AnalysisLimits {
                    maximum_cycle_length: 0,
                    maximum_cycles: 1,
                },
            )
            .await
            .unwrap_err();
        assert!(error.advice().iter().any(|item| item.contains("positive")));
        server.abort();
    }
}
