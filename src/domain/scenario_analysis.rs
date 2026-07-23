use super::{
    AnalysisRevisionKey, Edge, Node, Scenario, ScenarioAnalysis, ScenarioAnalysisError,
    scenario_analysis_graph::AnalysisGraph, scenario_analysis_sampling,
};

impl ScenarioAnalysis {
    /// Propagates every candidate intervention over a finite synchronous horizon.
    ///
    /// For state $i$, sampled baseline $b_i$, persistent intervention shift $u_i(t)$,
    /// local response $\beta_{ji}$, and integer delay $d_{ji}\geq1$, each period applies
    /// $x_i(t)=\operatorname{clamp}_i(b_i+u_i(t)+\sum_j
    /// \beta_{ji}(x_j(t-d_{ji})-b_j))$, where counterfactual responses sample
    /// $\beta=\Delta y/\Delta x$ with destination-unit/source-unit dimension.
    /// $\operatorname{clamp}_i$ uses each quantity's declared support. The one-period minimum
    /// delay makes updates
    /// synchronous: a zero-lag edge consumes its source at $t-1$, while explicit
    /// duration and lag samples are interpreted as planning periods, rounded up,
    /// and added to that one-period transport delay.
    ///
    /// Each candidate is evaluated independently. One pinned ChaCha20 stream samples
    /// every primitive once per joint draw, currently under explicit independence.
    /// Online Pébay/Welford moments retain no draws and report objective improvement
    /// covariance. Sampling stops after the configured minimum when every baseline,
    /// final-state, and improvement mean satisfies $SE(\bar X)\leq a+r|\bar X|$, or
    /// at the attempt limit. Monte Carlo standard errors quantify simulation noise,
    /// not model uncertainty or causal-identification error. Results are reproducible
    /// for the same immutable revision, seed, algorithm version, and pinned dependency
    /// versions; adding or reordering sampled primitives changes the random stream.
    ///
    /// The model is a finite-horizon baseline-delta approximation, not an equilibrium
    /// or causal-identification claim. Project-level dependence, intervention bundles,
    /// prerequisites, costs, conflicts, synergies, and scalar utility are excluded.
    /// See Sterman, *Business Dynamics*, chapters 6 and 13, for discrete-time stock/
    /// feedback simulation, and Pébay, SAND2008-6212, for online joint moments.
    pub fn compute(
        revision: AnalysisRevisionKey,
        scenario: &Scenario,
        nodes: &[Node],
        edges: &[Edge],
    ) -> Result<Self, ScenarioAnalysisError> {
        if revision.scenario != Some((scenario.id, scenario.revision)) {
            return Err(ScenarioAnalysisError::RevisionMismatch(scenario.id));
        }
        let graph = AnalysisGraph::new(scenario, nodes, edges)?;
        let candidates = scenario
            .draft
            .candidate_interventions
            .iter()
            .map(|candidate| {
                scenario_analysis_sampling::project_candidate(&graph, *candidate, scenario)
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            revision,
            planning_horizon: scenario.draft.planning_horizon,
            candidates,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        CausalEffect, Distribution, EdgePayload, EntityId, Estimate, EstimateId, Factor,
        Intervention, LinearResponse, Metric, MonteCarloConfig, NodeKind, NodePayload, Outcome,
        OutcomeDirection, ProjectId, QuantityDefinition, QuantityState, QuantitySupport,
        QuantityValue, ScenarioDraft, ScenarioId, ScenarioObjective, Unit, UtilityDirection,
    };

    fn estimate<T: super::super::EstimateDimension>(id: u64, value: f64) -> Estimate<T> {
        Estimate::new(EstimateId::new(id), Distribution::point(value).unwrap()).unwrap()
    }

    fn with_state(mut node: Node, value: f64) -> Node {
        node.native_state = Some(
            QuantityState::new(
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
                Some(estimate::<QuantityValue>(0, value)),
                None,
            )
            .unwrap(),
        );
        node
    }

    #[test]
    fn propagates_a_point_intervention_over_synchronous_periods() {
        let intervention = Node::new(
            EntityId::new(0),
            "small-batches",
            "Small batches",
            NodePayload::Intervention(Intervention {
                costs: vec![],
                duration: None,
                probability_of_success: Some(estimate(0, 1.0)),
                acceptance_criteria: vec![],
            }),
        )
        .unwrap();
        let factor = with_state(
            Node::new(
                EntityId::new(1),
                "feedback",
                "Feedback",
                NodePayload::Factor(Factor {
                    controllable: true,
                    evidence: vec![],
                }),
            )
            .unwrap(),
            0.5,
        );
        let outcome = with_state(
            Node::new(
                EntityId::new(2),
                "delivery",
                "Delivery",
                NodePayload::Outcome(Outcome {
                    direction: OutcomeDirection::Maximize,
                    evidence: vec![],
                }),
            )
            .unwrap(),
            0.5,
        );
        let changes = Edge::new(
            intervention.id,
            NodeKind::Intervention,
            factor.id,
            NodeKind::Factor,
            EdgePayload::Changes(
                CausalEffect::linear(
                    LinearResponse {
                        source_change: 1.0,
                        source_unit: Unit::dimensionless(),
                        destination_change: estimate::<QuantityValue>(0, 0.3),
                        destination_unit: Unit::dimensionless(),
                    },
                    None,
                    String::new(),
                    vec![],
                )
                .unwrap(),
            ),
        )
        .unwrap();
        let contributes = Edge::new(
            factor.id,
            NodeKind::Factor,
            outcome.id,
            NodeKind::Outcome,
            EdgePayload::Contributes(
                CausalEffect::linear(
                    LinearResponse {
                        source_change: 1.0,
                        source_unit: Unit::dimensionless(),
                        destination_change: estimate::<QuantityValue>(0, 0.2),
                        destination_unit: Unit::dimensionless(),
                    },
                    None,
                    String::new(),
                    vec![],
                )
                .unwrap(),
            ),
        )
        .unwrap();
        let scenario = Scenario::new(
            ScenarioId::new(0),
            ScenarioDraft {
                name: "plan".to_owned(),
                title: "Plan".to_owned(),
                rationale: String::new(),
                objectives: vec![ScenarioObjective {
                    outcome_id: outcome.id,
                    direction: UtilityDirection::Maximize,
                    importance: 1.0,
                }],
                planning_horizon: 2,
                budgets: vec![],
                candidate_interventions: vec![intervention.id],
                monte_carlo: MonteCarloConfig::new(7, 2, 2, 0.001, 0.0).unwrap(),
                scalar_preferences: None,
            },
        )
        .unwrap();
        let revision = AnalysisRevisionKey {
            project: ProjectId::new("analysis").unwrap(),
            graph_revision: 4,
            scenario: Some((scenario.id, scenario.revision)),
            dependence_revision: None,
            formula_revision: 0,
        };
        let result = ScenarioAnalysis::compute(
            revision,
            &scenario,
            &[intervention, factor, outcome],
            &[changes, contributes],
        )
        .unwrap();
        let objective = &result.candidates[0].objectives[0];
        assert!(objective.reachable);
        assert_eq!(objective.baseline.mean, Some(0.5));
        assert!((objective.final_state.mean.unwrap() - 0.56).abs() < 1e-12);
        assert!((objective.improvement.mean.unwrap() - 0.06).abs() < 1e-12);
        assert_eq!(result.candidates[0].clamped_state_updates, 0);
    }

    #[test]
    fn applies_causal_lags_in_integer_planning_periods() {
        let (scenario, nodes, mut edges, revision) = point_fixture(3);
        let EdgePayload::Contributes(effect) = &mut edges[1].payload else {
            unreachable!()
        };
        effect.lag = Some(estimate(1, 1.0));
        let result = ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges).unwrap();
        assert!(
            (result.candidates[0].objectives[0].final_state.mean.unwrap() - 0.56).abs() < 1e-12
        );

        let (scenario, nodes, edges, revision) = point_fixture(2);
        let mut delayed = edges;
        let EdgePayload::Contributes(effect) = &mut delayed[1].payload else {
            unreachable!()
        };
        effect.lag = Some(estimate(1, 1.0));
        let result = ScenarioAnalysis::compute(revision, &scenario, &nodes, &delayed).unwrap();
        assert_eq!(
            result.candidates[0].objectives[0].final_state.mean,
            Some(0.5)
        );
    }

    #[test]
    fn ignores_disconnected_incomplete_fragments_but_rejects_relevant_ones() {
        let (mut scenario, mut nodes, mut edges, revision) = point_fixture(2);
        let unrelated = Node::new(
            EntityId::new(3),
            "unrelated",
            "Unrelated",
            NodePayload::Factor(Factor {
                controllable: false,
                evidence: vec![],
            }),
        )
        .unwrap();
        let unrelated_outcome = with_state(
            Node::new(
                EntityId::new(4),
                "unrelated-outcome",
                "Unrelated outcome",
                NodePayload::Outcome(Outcome {
                    direction: OutcomeDirection::Maximize,
                    evidence: vec![],
                }),
            )
            .unwrap(),
            0.2,
        );
        scenario.draft.objectives.push(ScenarioObjective {
            outcome_id: unrelated_outcome.id,
            direction: UtilityDirection::Maximize,
            importance: 1.0,
        });
        edges.push(contributes(unrelated.id, unrelated_outcome.id, 0.5));
        nodes.extend([unrelated, unrelated_outcome]);
        let result =
            ScenarioAnalysis::compute(revision.clone(), &scenario, &nodes, &edges).unwrap();
        assert!(result.candidates[0].objectives[0].reachable);
        assert!(!result.candidates[0].objectives[1].reachable);
        assert_eq!(
            result.candidates[0].objectives[1].improvement.mean,
            Some(0.0)
        );

        nodes[1].native_state.as_mut().unwrap().current = None;
        assert_eq!(
            ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges),
            Err(ScenarioAnalysisError::MissingFactorBaseline(EntityId::new(
                1
            )))
        );
    }

    #[test]
    fn stochastic_runs_are_reproducible_and_states_remain_bounded() {
        let (mut scenario, mut nodes, mut edges, revision) = point_fixture(2);
        scenario.draft.monte_carlo = MonteCarloConfig::new(91, 100, 100, 0.0001, 0.0).unwrap();
        nodes[2].native_state.as_mut().unwrap().current =
            Some(Estimate::new(EstimateId::new(0), Distribution::beta(8.0, 2.0).unwrap()).unwrap());
        let EdgePayload::Changes(effect) = &mut edges[0].payload else {
            unreachable!()
        };
        effect.response.destination_change = Estimate::new(
            EstimateId::new(0),
            Distribution::scaled_beta(2.0, 2.0, -1.0, 1.0).unwrap(),
        )
        .unwrap();
        let first = ScenarioAnalysis::compute(revision.clone(), &scenario, &nodes, &edges).unwrap();
        let second = ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges).unwrap();
        assert_eq!(first, second);
        let projection = &first.candidates[0].objectives[0];
        assert!((0.0..=1.0).contains(&projection.final_state.mean.unwrap()));
        assert_eq!(first.candidates[0].diagnostics.valid_samples, 100);

        let EdgePayload::Changes(effect) = &mut edges[0].payload else {
            unreachable!()
        };
        effect.response.destination_change = estimate(0, 1.0);
        let saturated =
            ScenarioAnalysis::compute(first.revision.clone(), &scenario, &nodes, &edges).unwrap();
        assert!(saturated.candidates[0].clamped_state_updates > 0);
    }

    #[test]
    fn propagates_native_metric_responses_between_normalized_states() {
        let (scenario, mut nodes, mut edges, revision) = point_fixture(3);
        let outcome = nodes.pop().unwrap();
        let metric = Node::new(
            EntityId::new(3),
            "lead_time",
            "Lead time",
            NodePayload::Metric(
                Metric::with_quantity(
                    QuantityDefinition::with_dimension(
                        "days",
                        Some(Unit::base("day").unwrap()),
                        None,
                        QuantitySupport::Bounded {
                            lower: 0.0,
                            upper: 30.0,
                        },
                    )
                    .unwrap(),
                    Some(estimate::<QuantityValue>(0, 10.0)),
                )
                .unwrap(),
            ),
        )
        .unwrap();
        let factor_to_metric = Edge::new(
            EntityId::new(1),
            NodeKind::Factor,
            metric.id,
            NodeKind::Metric,
            EdgePayload::Contributes(
                CausalEffect::linear(
                    LinearResponse {
                        source_change: 0.1,
                        source_unit: Unit::dimensionless(),
                        destination_change: estimate::<QuantityValue>(0, -2.0),
                        destination_unit: Unit::base("day").unwrap(),
                    },
                    None,
                    String::new(),
                    vec![],
                )
                .unwrap(),
            ),
        )
        .unwrap();
        let metric_to_outcome = Edge::new(
            metric.id,
            NodeKind::Metric,
            outcome.id,
            NodeKind::Outcome,
            EdgePayload::Contributes(
                CausalEffect::linear(
                    LinearResponse {
                        source_change: -2.0,
                        source_unit: Unit::base("day").unwrap(),
                        destination_change: estimate::<QuantityValue>(0, 0.1),
                        destination_unit: Unit::dimensionless(),
                    },
                    None,
                    String::new(),
                    vec![],
                )
                .unwrap(),
            ),
        )
        .unwrap();
        nodes.push(metric);
        nodes.push(outcome);
        edges.pop();
        edges.extend([factor_to_metric, metric_to_outcome]);

        let result = ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges).unwrap();
        let objective = &result.candidates[0].objectives[0];
        assert!(objective.reachable);
        assert_eq!(objective.baseline.mean, Some(0.5));
        assert!((objective.final_state.mean.unwrap() - 0.8).abs() < 1e-12);
        assert!((objective.improvement.mean.unwrap() - 0.3).abs() < 1e-12);
        assert_eq!(result.candidates[0].clamped_state_updates, 0);
    }

    #[test]
    fn propagates_unit_aware_intervention_shifts_into_metrics() {
        let (mut scenario, mut nodes, _, revision) = point_fixture(2);
        let outcome = nodes.pop().unwrap();
        let metric = Node::new(
            EntityId::new(3),
            "lead_time",
            "Lead time",
            NodePayload::Metric(
                Metric::with_quantity(
                    QuantityDefinition::with_dimension(
                        "days",
                        Some(Unit::base("day").unwrap()),
                        None,
                        QuantitySupport::Bounded {
                            lower: 0.0,
                            upper: 30.0,
                        },
                    )
                    .unwrap(),
                    Some(estimate::<QuantityValue>(0, 10.0)),
                )
                .unwrap(),
            ),
        )
        .unwrap();
        let intervention_to_metric = Edge::new(
            nodes[0].id,
            NodeKind::Intervention,
            metric.id,
            NodeKind::Metric,
            EdgePayload::Changes(
                CausalEffect::linear(
                    LinearResponse {
                        source_change: 2.0,
                        source_unit: Unit::dimensionless(),
                        destination_change: estimate::<QuantityValue>(0, -2.0),
                        destination_unit: Unit::base("day").unwrap(),
                    },
                    None,
                    String::new(),
                    vec![],
                )
                .unwrap(),
            ),
        )
        .unwrap();
        let metric_to_outcome = Edge::new(
            metric.id,
            NodeKind::Metric,
            outcome.id,
            NodeKind::Outcome,
            EdgePayload::Contributes(
                CausalEffect::linear(
                    LinearResponse {
                        source_change: -2.0,
                        source_unit: Unit::base("day").unwrap(),
                        destination_change: estimate::<QuantityValue>(0, 0.1),
                        destination_unit: Unit::dimensionless(),
                    },
                    None,
                    String::new(),
                    vec![],
                )
                .unwrap(),
            ),
        )
        .unwrap();
        scenario.draft.planning_horizon = 2;
        nodes.push(metric);
        nodes.push(outcome);
        let edges = vec![intervention_to_metric, metric_to_outcome];

        let result = ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges).unwrap();
        let objective = &result.candidates[0].objectives[0];
        assert!(objective.reachable);
        assert!((objective.final_state.mean.unwrap() - 0.55).abs() < 1e-12);
        assert!((objective.improvement.mean.unwrap() - 0.05).abs() < 1e-12);
        let expected = [(0, 0.5, 0.0), (1, 0.5, 0.0), (2, 0.55, 0.05)];
        for (point, (period, state, improvement)) in objective.trajectory.iter().zip(expected) {
            assert_eq!(point.period, period);
            assert!((point.state.mean.unwrap() - state).abs() < 1e-12);
            assert!((point.improvement.mean.unwrap() - improvement).abs() < 1e-12);
        }
    }

    fn point_fixture(
        planning_horizon: u64,
    ) -> (Scenario, Vec<Node>, Vec<Edge>, AnalysisRevisionKey) {
        let intervention = Node::new(
            EntityId::new(0),
            "small-batches",
            "Small batches",
            NodePayload::Intervention(Intervention {
                costs: vec![],
                duration: None,
                probability_of_success: Some(estimate(0, 1.0)),
                acceptance_criteria: vec![],
            }),
        )
        .unwrap();
        let factor = with_state(
            Node::new(
                EntityId::new(1),
                "feedback",
                "Feedback",
                NodePayload::Factor(Factor {
                    controllable: true,
                    evidence: vec![],
                }),
            )
            .unwrap(),
            0.5,
        );
        let outcome = with_state(
            Node::new(
                EntityId::new(2),
                "delivery",
                "Delivery",
                NodePayload::Outcome(Outcome {
                    direction: OutcomeDirection::Maximize,
                    evidence: vec![],
                }),
            )
            .unwrap(),
            0.5,
        );
        let changes = Edge::new(
            intervention.id,
            NodeKind::Intervention,
            factor.id,
            NodeKind::Factor,
            EdgePayload::Changes(
                CausalEffect::linear(
                    LinearResponse {
                        source_change: 1.0,
                        source_unit: Unit::dimensionless(),
                        destination_change: estimate::<QuantityValue>(0, 0.3),
                        destination_unit: Unit::dimensionless(),
                    },
                    None,
                    String::new(),
                    vec![],
                )
                .unwrap(),
            ),
        )
        .unwrap();
        let contributes = contributes(factor.id, outcome.id, 0.2);
        let scenario = Scenario::new(
            ScenarioId::new(0),
            ScenarioDraft {
                name: "plan".to_owned(),
                title: "Plan".to_owned(),
                rationale: String::new(),
                objectives: vec![ScenarioObjective {
                    outcome_id: outcome.id,
                    direction: UtilityDirection::Maximize,
                    importance: 1.0,
                }],
                planning_horizon,
                budgets: vec![],
                candidate_interventions: vec![intervention.id],
                monte_carlo: MonteCarloConfig::new(7, 2, 2, 0.001, 0.0).unwrap(),
                scalar_preferences: None,
            },
        )
        .unwrap();
        let revision = AnalysisRevisionKey {
            project: ProjectId::new("analysis").unwrap(),
            graph_revision: 4,
            scenario: Some((scenario.id, scenario.revision)),
            dependence_revision: None,
            formula_revision: 0,
        };
        (
            scenario,
            vec![intervention, factor, outcome],
            vec![changes, contributes],
            revision,
        )
    }

    fn contributes(source: EntityId, destination: EntityId, effect: f64) -> Edge {
        Edge::new(
            source,
            NodeKind::Factor,
            destination,
            NodeKind::Outcome,
            EdgePayload::Contributes(
                CausalEffect::linear(
                    LinearResponse {
                        source_change: 1.0,
                        source_unit: Unit::dimensionless(),
                        destination_change: estimate::<QuantityValue>(0, effect),
                        destination_unit: Unit::dimensionless(),
                    },
                    None,
                    String::new(),
                    vec![],
                )
                .unwrap(),
            ),
        )
        .unwrap()
    }
}
