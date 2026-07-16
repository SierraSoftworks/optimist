use optimist::domain::{
    AnalysisLimits, AnalysisRevisionKey, CausalEffect, Distribution, Edge, EdgePayload, EntityId,
    Estimate, EstimateId, Factor, Node, NodeKind, NodePayload, ProjectId, SignedInfluence,
    StructuralAnalysis,
};

fn factor(id: u64, name: &str, title: &str) -> Node {
    Node::new(
        EntityId::new(id),
        name,
        title,
        NodePayload::Factor(Factor {
            current: None,
            desired: None,
            controllable: false,
            evidence: vec![],
        }),
    )
    .expect("example factors are valid")
}

fn contributes(source: u64, destination: u64, estimate_id: u64) -> Edge {
    Edge::new(
        EntityId::new(source),
        NodeKind::Factor,
        EntityId::new(destination),
        NodeKind::Factor,
        EdgePayload::Contributes(CausalEffect {
            effect: Estimate::<SignedInfluence>::new(
                EstimateId::new(estimate_id),
                Distribution::scaled_beta(8.0, 2.0, 0.0, 1.0)
                    .expect("positive influence distribution is valid"),
            )
            .expect("distribution fits signed influence support"),
            lag: None,
            mechanism: "Improvement reinforces the next delivery capability.".to_owned(),
            evidence: vec!["Team retrospective".to_owned()],
        }),
    )
    .expect("factor-to-factor contribution is valid")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let nodes = vec![
        factor(0, "delivery_speed", "Delivery speed"),
        factor(1, "feedback_quality", "Feedback quality"),
        factor(2, "learning_rate", "Learning rate"),
    ];
    let edges = vec![
        contributes(0, 1, 0),
        contributes(1, 2, 0),
        contributes(2, 0, 0),
    ];
    let analysis = StructuralAnalysis::compute(
        AnalysisRevisionKey {
            project: ProjectId::new("delivery")?,
            graph_revision: 1,
            scenario: None,
            dependence_revision: None,
            formula_revision: 0,
        },
        &nodes,
        &edges,
        AnalysisLimits::new(6, 100)?,
    )?;

    let feedback_components: Vec<_> = analysis
        .components
        .iter()
        .filter(|component| component.is_feedback)
        .collect();
    assert_eq!(feedback_components.len(), 1);
    assert_eq!(analysis.cycles.len(), 1);

    println!("Feedback component: {:?}", feedback_components[0].nodes);
    println!("Elementary cycle: {:?}", analysis.cycles[0].edges);
    Ok(())
}
