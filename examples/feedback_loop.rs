use optimist::domain::{
    AnalysisLimits, AnalysisRevisionKey, CausalEffect, Edge, EdgePayload, EntityId, Estimate,
    EstimateId, Factor, LinearResponse, Node, NodeKind, NodePayload, ProjectId, QuantityDefinition,
    QuantityState, QuantitySupport, QuantityValue, SquiggleEstimateDefinition, StructuralAnalysis,
    Unit,
};

fn factor(id: u64, name: &str, title: &str) -> Node {
    let mut node = Node::new(
        EntityId::new(id),
        name,
        title,
        NodePayload::Factor(Factor {
            controllable: false,
            evidence: vec![],
        }),
    )
    .expect("example factors are valid");
    node.native_state = Some(
        QuantityState::new(
            QuantityDefinition::with_dimension(
                "score",
                Some(Unit::base("score").expect("valid unit")),
                None,
                QuantitySupport::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
            )
            .expect("valid quantity"),
            None,
            None,
        )
        .expect("valid state"),
    );
    node
}

fn contributes(source: u64, destination: u64, estimate_id: u64) -> Edge {
    Edge::new(
        EntityId::new(source),
        NodeKind::Factor,
        EntityId::new(destination),
        NodeKind::Factor,
        EdgePayload::Contributes(
            CausalEffect::linear(
                LinearResponse {
                    source_change: 1.0,
                    source_unit: Unit::base("score").expect("valid unit"),
                    destination_change: Estimate::<QuantityValue>::from_squiggle(
                        EstimateId::new(estimate_id),
                        SquiggleEstimateDefinition {
                            source: "beta(8, 2)".to_owned(),
                            seed: 42,
                            sample_count: 256,
                            target_unit: Unit::base("score").expect("valid unit"),
                        },
                        &Unit::base("score").expect("valid unit"),
                    )
                    .expect("valid response estimate"),
                    destination_unit: Unit::base("score").expect("valid unit"),
                },
                None,
                "Improvement reinforces the next delivery capability.".to_owned(),
                vec!["Team retrospective".to_owned()],
            )
            .expect("valid causal response"),
        ),
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
