use super::{
    AnalysisRevisionKey, Edge, Node, ProjectDependenceModel, Scenario, ScenarioAnalysis,
    ScenarioAnalysisError, scenario_analysis_candidates, scenario_analysis_graph::AnalysisGraph,
    scenario_analysis_stability,
};

impl ScenarioAnalysis {
    /// Propagates every candidate intervention over a finite synchronous horizon.
    ///
    /// Responses are dimensionless proportional claims, so a state moves relative to
    /// its own sampled baseline rather than by an amount expressed in its unit. The
    /// combination rule follows each quantity's declared support. A strictly
    /// non-negative state composes multiplicatively,
    /// $x_i(t)=\operatorname{clamp}_i\left(b_i\prod_j\left(\frac{x_j(t-d_{ji})}{b_j}\right)^{\varepsilon_{ji}}\right)$,
    /// which stays non-negative for free and makes a plain product expressible with
    /// unit elasticities. A state that may be zero or negative has no ratio scale, so
    /// its responses accumulate,
    /// $x_i(t)=\operatorname{clamp}_i\left(b_i\left(1+\sum_j\varepsilon_{ji}\left(\frac{x_j(t-d_{ji})}{b_j}-1\right)\right)\right)$.
    /// A source whose sampled baseline is zero has no fractional movement, so its
    /// responses are dropped and reported as undefined rather than propagating an
    /// infinity. The one-period minimum
    /// delay makes updates
    /// synchronous: a zero-lag edge consumes its source at $t-1$, while explicit
    /// duration and lag samples are interpreted as planning periods, rounded up,
    /// and added to that one-period transport delay.
    ///
    /// A `changes` effect has no source level to take a ratio of, so its response is
    /// the multiplier $m_k$ applied while the intervention is fully active, and its
    /// temporal activation $a_k(t)$ enters as the exponent: it contributes
    /// $m_k^{a_k(t)}$ multiplicatively, or the share $(m_k-1)a_k(t)$ additively, with
    /// the sampled rebound magnitude $\rho_k$ applied the same way against $b_k(t)$.
    /// Effects without a profile hold $a_k=1$ and
    /// $b_k=0$ after arrival, which is the monotone step a permanent intervention applies.
    /// Shaping an effect therefore changes only its schedule, never its magnitude.
    ///
    /// Each candidate execution plan is evaluated independently. Required interventions
    /// execute first; durations add and every required success gates later steps. One
    /// pinned ChaCha20 stream samples every primitive once per joint draw.
    ///
    /// Estimates named by the project's residual dependence document are not drawn from
    /// that stream. Each group's Gaussian copula is drawn first, and its members take
    /// their values by inverse transform $x=F^{-1}(u)$, which reproduces every authored
    /// marginal exactly while carrying the stated correlation. Coupling is an asserted
    /// residual relationship rather than one inferred from the graph, and a group whose
    /// members this scenario never samples still consumes its draw so member positions
    /// stay aligned with the matrix. A project without groups consumes no extra
    /// randomness and reproduces results from before it had a dependence document.
    ///
    /// Online Pébay/Welford moments retain no draws and report objective improvement
    /// covariance. Sampling stops after the configured minimum when every baseline,
    /// final-state, and improvement mean satisfies $SE(\bar X)\leq a+r|\bar X|$, or
    /// at the attempt limit. Monte Carlo standard errors quantify simulation noise,
    /// not model uncertainty or causal-identification error. Results are reproducible
    /// for the same immutable revision, seed, algorithm version, and pinned dependency
    /// versions; adding or reordering sampled primitives changes the random stream.
    ///
    /// The model is a finite-horizon baseline-delta approximation, not an equilibrium
    /// or causal-identification claim. Intervention bundles, costs, numeric synergy
    /// effects, and scalar utility are excluded.
    /// See Sterman, *Business Dynamics*, chapters 6 and 13, for discrete-time stock/
    /// feedback simulation, Nelsen, *An Introduction to Copulas*, chapter 5, for the
    /// inverse-transform construction, and Pébay, SAND2008-6212, for online joint moments.
    pub fn compute(
        revision: AnalysisRevisionKey,
        scenario: &Scenario,
        nodes: &[Node],
        edges: &[Edge],
        dependence: Option<&ProjectDependenceModel>,
    ) -> Result<Self, ScenarioAnalysisError> {
        if revision.scenario != Some((scenario.id, scenario.revision)) {
            return Err(ScenarioAnalysisError::RevisionMismatch(scenario.id));
        }
        let graph = AnalysisGraph::new(&revision.project, scenario, nodes, edges, dependence)?;
        let candidates = scenario_analysis_candidates::project_candidates(&graph, scenario)?;
        Ok(Self {
            revision,
            planning_horizon: scenario.draft.planning_horizon,
            candidates,
            feedback_loops: scenario_analysis_stability::feedback_loops(
                nodes,
                edges,
                &graph.states,
                &graph.propagation_edges,
                &graph.coupling,
                scenario.draft.monte_carlo.seed(),
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        CausalEffect, CorrelationScale, Distribution, Duration, EdgePayload, EffectAftereffect,
        EffectProfile, EffectRelease, EffectTransience, Elasticity, EntityId, Estimate,
        EstimateAddress, EstimateId, EstimateOwner, Factor, GaussianCopulaCorrelation,
        Intervention, Metric, MonteCarloConfig, NodeKind, NodePayload, Outcome, OutcomeDirection,
        ProjectDependenceModel, ProjectId, QuantityDefinition, QuantityState, QuantitySupport,
        QuantityValue, Requirement, ResidualDependenceGroup, ScenarioDraft, ScenarioId,
        ScenarioObjective, StateRelation, Unit, UtilityDirection,
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
            EdgePayload::Changes(CausalEffect::proportional(
                estimate::<Elasticity>(0, 1.6),
                None,
                String::new(),
                vec![],
            )),
        )
        .unwrap();
        let contributes = Edge::new(
            factor.id,
            NodeKind::Factor,
            outcome.id,
            NodeKind::Outcome,
            EdgePayload::Contributes(CausalEffect::proportional(
                estimate::<Elasticity>(0, 0.2),
                None,
                String::new(),
                vec![],
            )),
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
        };
        let result = ScenarioAnalysis::compute(
            revision,
            &scenario,
            &[intervention, factor, outcome],
            &[changes, contributes],
            None,
        )
        .unwrap();
        let objective = &result.candidates[0].objectives[0];
        assert!(objective.reachable);
        assert_eq!(objective.baseline.mean, Some(0.5));
        assert!((objective.final_state.mean.unwrap() - 0.56).abs() < 1e-12);
        assert!((objective.improvement.mean.unwrap() - 0.06).abs() < 1e-12);
        assert_eq!(result.candidates[0].clamped_state_updates, 0);
    }

    /// Candidates are projected concurrently, so the split must be invisible.
    ///
    /// Each candidate seeds its own stream from the scenario seed, which is what
    /// makes them independent; this pins that property down by projecting the
    /// same candidates alone and requiring every number to match exactly, and by
    /// requiring the projections to arrive in the order the scenario lists them
    /// rather than in the order the workers happen to finish.
    #[test]
    fn projects_candidates_concurrently_without_changing_a_draw() {
        let (mut scenario, mut nodes, mut edges, mut revision) = point_fixture(4);
        let factor = nodes[1].id;
        for (offset, effect) in [(3_u64, 1.1), (4, 1.3), (5, 1.5), (6, 1.7)] {
            let intervention = Node::new(
                EntityId::new(offset),
                format!("lever-{offset}"),
                format!("Lever {offset}"),
                NodePayload::Intervention(Intervention {
                    costs: vec![],
                    duration: None,
                    probability_of_success: Some(estimate(0, 1.0)),
                    acceptance_criteria: vec![],
                }),
            )
            .unwrap();
            edges.push(
                Edge::new(
                    intervention.id,
                    NodeKind::Intervention,
                    factor,
                    NodeKind::Factor,
                    EdgePayload::Changes(CausalEffect::proportional(
                        estimate::<Elasticity>(0, effect),
                        None,
                        String::new(),
                        vec![],
                    )),
                )
                .unwrap(),
            );
            scenario.draft.candidate_interventions.push(intervention.id);
            nodes.push(intervention);
        }
        revision.graph_revision += 1;

        let together =
            ScenarioAnalysis::compute(revision.clone(), &scenario, &nodes, &edges, None).unwrap();
        assert_eq!(
            together
                .candidates
                .iter()
                .map(|candidate| candidate.intervention)
                .collect::<Vec<_>>(),
            scenario.draft.candidate_interventions,
        );
        for (index, candidate) in scenario.draft.candidate_interventions.iter().enumerate() {
            let mut alone = scenario.clone();
            alone.draft.candidate_interventions = vec![*candidate];
            let separately =
                ScenarioAnalysis::compute(revision.clone(), &alone, &nodes, &edges, None).unwrap();
            assert_eq!(together.candidates[index], separately.candidates[0]);
        }
    }

    #[test]
    fn executes_required_interventions_before_the_candidate() {
        let (mut scenario, mut nodes, mut edges, mut revision) = point_fixture(6);
        let candidate = nodes[0].id;
        let prerequisite = Node::new(
            EntityId::new(3),
            "foundation",
            "Foundation",
            NodePayload::Intervention(Intervention {
                costs: vec![],
                duration: Some(estimate(0, 1.0)),
                probability_of_success: Some(estimate(1, 1.0)),
                acceptance_criteria: vec![],
            }),
        )
        .unwrap();
        let NodePayload::Intervention(candidate_value) = &mut nodes[0].payload else {
            unreachable!()
        };
        candidate_value.duration = Some(estimate(2, 2.0));
        let prerequisite_change = Edge::new(
            prerequisite.id,
            NodeKind::Intervention,
            nodes[1].id,
            NodeKind::Factor,
            EdgePayload::Changes(CausalEffect::proportional(
                estimate::<Elasticity>(0, 1.2),
                None,
                String::new(),
                vec![],
            )),
        )
        .unwrap();
        let requires = Edge::new(
            candidate,
            NodeKind::Intervention,
            prerequisite.id,
            NodeKind::Intervention,
            EdgePayload::Requires(Requirement {
                hard: true,
                satisfaction_threshold: None,
            }),
        )
        .unwrap();
        nodes.push(prerequisite);
        edges.extend([prerequisite_change, requires]);
        scenario.draft.monte_carlo = MonteCarloConfig::new(7, 2, 2, 0.001, 0.0).unwrap();
        revision.graph_revision += 2;

        let result = ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges, None).unwrap();
        let projection = &result.candidates[0];
        assert_eq!(projection.prerequisites, vec![EntityId::new(3)]);
        assert_eq!(projection.execution_duration.mean, Some(3.0));
        assert_eq!(projection.execution_success.mean, Some(1.0));
        assert!(projection.objectives[0].improvement.mean.unwrap() > 0.06);
    }

    #[test]
    fn applies_causal_lags_in_integer_planning_periods() {
        let (scenario, nodes, mut edges, revision) = point_fixture(3);
        let EdgePayload::Contributes(effect) = &mut edges[1].payload else {
            unreachable!()
        };
        effect.lag = Some(estimate(1, 1.0));
        let result = ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges, None).unwrap();
        assert!(
            (result.candidates[0].objectives[0].final_state.mean.unwrap() - 0.56).abs() < 1e-12
        );

        let (scenario, nodes, edges, revision) = point_fixture(2);
        let mut delayed = edges;
        let EdgePayload::Contributes(effect) = &mut delayed[1].payload else {
            unreachable!()
        };
        effect.lag = Some(estimate(1, 1.0));
        let result =
            ScenarioAnalysis::compute(revision, &scenario, &nodes, &delayed, None).unwrap();
        assert_eq!(
            result.candidates[0].objectives[0].final_state.mean,
            Some(0.5)
        );
    }

    /// Builds a two-period pulse that ends abruptly, optionally with a rebound.
    fn pulse(aftereffect: Option<EffectAftereffect>) -> EffectProfile {
        EffectProfile::new(
            None,
            Some(estimate::<Duration>(1, 2.0)),
            EffectRelease::Immediate,
            aftereffect,
        )
        .unwrap()
    }

    fn shaped(
        planning_horizon: u64,
        rebound: Option<Estimate<Elasticity>>,
        profile: EffectProfile,
    ) -> ScenarioAnalysis {
        let (scenario, nodes, mut edges, revision) = point_fixture(planning_horizon);
        let EdgePayload::Changes(effect) = &mut edges[0].payload else {
            unreachable!("the fixture's first edge is an intervention effect")
        };
        *effect = effect
            .clone()
            .with_transience(Some(EffectTransience::new(profile, rebound).unwrap()));
        ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges, None).unwrap()
    }

    #[test]
    fn reverts_a_time_boxed_intervention_after_its_hold_window() {
        let (scenario, nodes, edges, revision) = point_fixture(4);
        let persistent =
            ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges, None).unwrap();
        let held = persistent.candidates[0].objectives[0]
            .final_state
            .mean
            .unwrap();
        assert!(
            (held - 0.56).abs() < 1e-12,
            "an unshaped effect must persist to the horizon"
        );

        let inside = shaped(3, None, pulse(None));
        assert!(
            (inside.candidates[0].objectives[0].final_state.mean.unwrap() - 0.56).abs() < 1e-12,
            "the outcome must still be moved while the pulse is held"
        );

        let outside = shaped(4, None, pulse(None));
        let objective = &outside.candidates[0].objectives[0];
        assert!(
            (objective.final_state.mean.unwrap() - 0.5).abs() < 1e-12,
            "the outcome must return to baseline once the pulse releases"
        );
        assert!(objective.improvement.mean.unwrap().abs() < 1e-12);
    }

    #[test]
    fn applies_a_rebound_when_a_time_boxed_intervention_ends() {
        let result = shaped(
            4,
            Some(estimate::<Elasticity>(3, 0.8)),
            pulse(Some(EffectAftereffect {
                hold: Some(estimate::<Duration>(2, 1.0)),
                release: EffectRelease::Immediate,
            })),
        );
        let objective = &result.candidates[0].objectives[0];
        assert!(
            (objective.final_state.mean.unwrap() - 0.48).abs() < 1e-12,
            "the rebound must overshoot past the original baseline"
        );
        assert!((objective.improvement.mean.unwrap() + 0.02).abs() < 1e-12);
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
            ScenarioAnalysis::compute(revision.clone(), &scenario, &nodes, &edges, None).unwrap();
        assert!(result.candidates[0].objectives[0].reachable);
        assert!(!result.candidates[0].objectives[1].reachable);
        assert_eq!(
            result.candidates[0].objectives[1].improvement.mean,
            Some(0.0)
        );

        nodes[1].native_state.as_mut().unwrap().current = None;
        assert_eq!(
            ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges, None),
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
        effect.response = Estimate::new(
            EstimateId::new(0),
            Distribution::scaled_beta(2.0, 2.0, 0.0, 2.0).unwrap(),
        )
        .unwrap();
        let first =
            ScenarioAnalysis::compute(revision.clone(), &scenario, &nodes, &edges, None).unwrap();
        let second = ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges, None).unwrap();
        assert_eq!(first, second);
        let projection = &first.candidates[0].objectives[0];
        assert!((0.0..=1.0).contains(&projection.final_state.mean.unwrap()));
        assert_eq!(first.candidates[0].diagnostics.valid_samples, 100);

        let EdgePayload::Changes(effect) = &mut edges[0].payload else {
            unreachable!()
        };
        effect.response = estimate(0, 3.0);
        let saturated =
            ScenarioAnalysis::compute(first.revision.clone(), &scenario, &nodes, &edges, None)
                .unwrap();
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
            EdgePayload::Contributes(CausalEffect::proportional(
                estimate::<Elasticity>(0, -0.5),
                None,
                String::new(),
                vec![],
            )),
        )
        .unwrap();
        let metric_to_outcome = Edge::new(
            metric.id,
            NodeKind::Metric,
            outcome.id,
            NodeKind::Outcome,
            EdgePayload::Contributes(CausalEffect::proportional(
                estimate::<Elasticity>(0, -1.0),
                None,
                String::new(),
                vec![],
            )),
        )
        .unwrap();
        nodes.push(metric);
        nodes.push(outcome);
        edges.pop();
        edges.extend([factor_to_metric, metric_to_outcome]);

        let result = ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges, None).unwrap();
        let objective = &result.candidates[0].objectives[0];
        assert!(objective.reachable);
        assert_eq!(objective.baseline.mean, Some(0.5));
        assert!((objective.final_state.mean.unwrap() - 0.65).abs() < 1e-12);
        assert!((objective.improvement.mean.unwrap() - 0.15).abs() < 1e-12);
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
            EdgePayload::Changes(CausalEffect::proportional(
                estimate::<Elasticity>(0, 0.7),
                None,
                String::new(),
                vec![],
            )),
        )
        .unwrap();
        let metric_to_outcome = Edge::new(
            metric.id,
            NodeKind::Metric,
            outcome.id,
            NodeKind::Outcome,
            EdgePayload::Contributes(CausalEffect::proportional(
                estimate::<Elasticity>(0, -1.0),
                None,
                String::new(),
                vec![],
            )),
        )
        .unwrap();
        scenario.draft.planning_horizon = 2;
        nodes.push(metric);
        nodes.push(outcome);
        let edges = vec![intervention_to_metric, metric_to_outcome];

        let result = ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges, None).unwrap();
        let objective = &result.candidates[0].objectives[0];
        assert!(objective.reachable);
        assert!((objective.final_state.mean.unwrap() - 0.65).abs() < 1e-12);
        assert!((objective.improvement.mean.unwrap() - 0.15).abs() < 1e-12);
        let expected = [(0, 0.5, 0.0), (1, 0.5, 0.0), (2, 0.65, 0.15)];
        for (point, (period, state, improvement)) in objective.trajectory.iter().zip(expected) {
            assert_eq!(point.period, period);
            assert!((point.state.mean.unwrap() - state).abs() < 1e-12);
            assert!((point.improvement.mean.unwrap() - improvement).abs() < 1e-12);
        }
    }

    /// Couples two objective baselines and checks the coupling reaches the result.
    ///
    /// Both objectives read the same factor through equal elasticities, so each
    /// improvement is a fixed multiple of its own baseline. Under a unit
    /// correlation the two baselines are one variable per draw, which makes the
    /// sampled improvement covariance exactly the sampled improvement variance —
    /// an identity that holds per draw and so carries no Monte Carlo error.
    #[test]
    fn coupled_baselines_move_together_without_changing_their_marginals() {
        let coupled = correlated_objectives(1.0);
        let (covariance, variance) = (
            coupled.candidates[0].improvement_covariance[0][1].unwrap(),
            coupled.candidates[0].objectives[0]
                .improvement
                .variance
                .unwrap(),
        );
        assert!(
            (covariance - variance).abs() < 1e-12,
            "perfectly coupled baselines must produce one shared improvement"
        );
        assert!(
            variance > 0.0,
            "the fixture must retain baseline uncertainty"
        );

        let independent = correlated_objectives(0.0);
        let uncoupled = independent.candidates[0].improvement_covariance[0][1].unwrap();
        assert!(
            uncoupled.abs() < covariance / 4.0,
            "independent baselines must not reproduce a coupled covariance"
        );
        for analysis in [&coupled, &independent] {
            for objective in &analysis.candidates[0].objectives {
                assert!(
                    (objective.baseline.mean.unwrap() - 0.4).abs() < 0.01,
                    "coupling must leave each authored marginal in place"
                );
            }
        }
    }

    /// Projects two objectives whose baselines share a copula of `correlation`.
    fn correlated_objectives(correlation: f64) -> ScenarioAnalysis {
        let (mut scenario, mut nodes, mut edges, revision) = point_fixture(2);
        let uncertain = |id: u64, name: &str, title: &str| {
            let node = Node::new(
                EntityId::new(id),
                name,
                title,
                NodePayload::Outcome(Outcome {
                    direction: OutcomeDirection::Maximize,
                    evidence: vec![],
                }),
            )
            .unwrap();
            let mut node = with_state(node, 0.4);
            node.native_state.as_mut().unwrap().current = Some(
                Estimate::new(
                    EstimateId::new(0),
                    Distribution::scaled_beta(2.0, 2.0, 0.2, 0.6).unwrap(),
                )
                .unwrap(),
            );
            node
        };
        nodes[2] = uncertain(2, "delivery", "Delivery");
        let second = uncertain(3, "retention", "Retention");
        edges.push(contributes(nodes[1].id, second.id, 0.2));
        scenario.draft.objectives.push(ScenarioObjective {
            outcome_id: second.id,
            direction: UtilityDirection::Maximize,
            importance: 1.0,
        });
        scenario.draft.monte_carlo = MonteCarloConfig::new(17, 400, 400, 1e-9, 0.0).unwrap();
        nodes.push(second);
        let dependence = ProjectDependenceModel {
            revision: 0,
            residual_groups: vec![ResidualDependenceGroup {
                members: [2, 3]
                    .map(|id| {
                        EstimateAddress::new(
                            revision.project.clone(),
                            EstimateOwner::Node(EntityId::new(id)),
                            EstimateId::new(0),
                        )
                    })
                    .to_vec(),
                correlation: GaussianCopulaCorrelation {
                    scale: CorrelationScale::Latent,
                    matrix: vec![vec![1.0, correlation], vec![correlation, 1.0]],
                },
            }],
        };
        ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges, Some(&dependence)).unwrap()
    }

    /// Replaces proportional composition with an authored node equation.
    ///
    /// The fixture's outcome is the product of two parents, which no single
    /// elasticity can express: a product needs both parents multiplied, not each
    /// scaled independently against a baseline. The equation states it directly,
    /// and the projection reproduces the arithmetic exactly.
    #[test]
    fn a_node_equation_replaces_proportional_composition() {
        let (mut scenario, mut nodes, _, revision) = point_fixture(3);
        let outcome = nodes.pop().unwrap();
        let frequency = measured(
            3,
            "outage_frequency",
            "Outage frequency",
            Unit::base("outage").unwrap(),
            4.0,
        );
        let duration = measured(
            4,
            "impact_duration",
            "Impact duration",
            Unit::from_exponents([("minute", 1), ("outage", -1)]).unwrap(),
            30.0,
        );
        let mut impact = Node::new(
            EntityId::new(5),
            "customer_impact",
            "Customer impact",
            NodePayload::Outcome(Outcome {
                direction: OutcomeDirection::Minimize,
                evidence: vec![],
            }),
        )
        .unwrap();
        impact.native_state = Some(
            QuantityState::new(
                QuantityDefinition::with_dimension(
                    "minutes",
                    Some(Unit::base("minute").unwrap()),
                    None,
                    QuantitySupport::NonNegative,
                )
                .unwrap(),
                Some(estimate::<QuantityValue>(0, 120.0)),
                None,
            )
            .unwrap()
            .with_relation(Some(
                StateRelation::new(
                    "outage_frequency * impact_duration".to_owned(),
                    Default::default(),
                )
                .unwrap(),
            )),
        );

        // The intervention halves outage frequency; nothing touches duration.
        let changes = Edge::new(
            nodes[0].id,
            NodeKind::Intervention,
            frequency.id,
            NodeKind::Metric,
            EdgePayload::Changes(CausalEffect::proportional(
                estimate::<Elasticity>(0, 0.5),
                None,
                String::new(),
                vec![],
            )),
        )
        .unwrap();
        let contributes = |source: EntityId| {
            Edge::new(
                source,
                NodeKind::Metric,
                impact.id,
                NodeKind::Outcome,
                EdgePayload::Contributes(CausalEffect::proportional(
                    estimate::<Elasticity>(0, 1.0),
                    None,
                    String::new(),
                    vec![],
                )),
            )
            .unwrap()
        };
        let edges = vec![changes, contributes(frequency.id), contributes(duration.id)];
        scenario.draft.objectives = vec![ScenarioObjective {
            outcome_id: impact.id,
            direction: UtilityDirection::Minimize,
            importance: 1.0,
        }];
        nodes.pop();
        nodes.extend([frequency, duration, impact, outcome]);

        let result = ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges, None).unwrap();
        let objective = &result.candidates[0].objectives[0];
        // The equation computes 4 x 30 from the very first period, so the stored
        // baseline of 120 is reproduced rather than assumed.
        assert!((objective.baseline.mean.unwrap() - 120.0).abs() < 1e-12);
        // Halving frequency halves the product: 2 x 30 = 60.
        assert!(
            (objective.final_state.mean.unwrap() - 60.0).abs() < 1e-12,
            "the equation must multiply its parents rather than scale each one"
        );
        assert!((objective.improvement.mean.unwrap() - 60.0).abs() < 1e-12);
    }

    /// Derives an equation-backed baseline from the equation, not the estimate.
    ///
    /// The fixture stores a deliberately stale current value on the outcome and
    /// on the metric between it and the root. Both equations must be settled in
    /// dependency order for the objective to read the root's 4 through the chain,
    /// and improvement must stay at zero: nothing intervenes, so a baseline drawn
    /// from anything other than the equation would manufacture a difference.
    #[test]
    fn an_equation_baseline_settles_through_a_chain_of_equations() {
        let (mut scenario, mut nodes, _, revision) = point_fixture(3);
        nodes.pop();
        let root = measured(
            3,
            "outage_frequency",
            "Outage frequency",
            Unit::base("outage").unwrap(),
            4.0,
        );
        let mut middle = measured(
            4,
            "weekly_outages",
            "Weekly outages",
            Unit::base("outage").unwrap(),
            999.0,
        );
        let NodePayload::Metric(metric) = &mut middle.payload else {
            panic!("expected a metric")
        };
        *metric = metric.clone().with_relation(Some(
            StateRelation::new("outage_frequency".to_owned(), Default::default()).unwrap(),
        ));
        let mut impact = Node::new(
            EntityId::new(5),
            "customer_impact",
            "Customer impact",
            NodePayload::Outcome(Outcome {
                direction: OutcomeDirection::Minimize,
                evidence: vec![],
            }),
        )
        .unwrap();
        impact.native_state = Some(
            QuantityState::new(
                QuantityDefinition::with_dimension(
                    "outages",
                    Some(Unit::base("outage").unwrap()),
                    None,
                    QuantitySupport::NonNegative,
                )
                .unwrap(),
                Some(estimate::<QuantityValue>(0, 999.0)),
                None,
            )
            .unwrap()
            .with_relation(Some(
                StateRelation::new("weekly_outages".to_owned(), Default::default()).unwrap(),
            )),
        );
        let link = |source: EntityId, source_kind, destination: EntityId, destination_kind| {
            Edge::new(
                source,
                source_kind,
                destination,
                destination_kind,
                EdgePayload::Contributes(CausalEffect::proportional(
                    estimate::<Elasticity>(0, 1.0),
                    None,
                    String::new(),
                    vec![],
                )),
            )
            .unwrap()
        };
        let edges = vec![
            link(root.id, NodeKind::Metric, middle.id, NodeKind::Metric),
            link(middle.id, NodeKind::Metric, impact.id, NodeKind::Outcome),
        ];
        scenario.draft.objectives = vec![ScenarioObjective {
            outcome_id: impact.id,
            direction: UtilityDirection::Minimize,
            importance: 1.0,
        }];
        nodes.extend([root, middle, impact]);

        let result = ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges, None).unwrap();
        let objective = &result.candidates[0].objectives[0];
        assert!(
            (objective.baseline.mean.unwrap() - 4.0).abs() < 1e-12,
            "the baseline must come from the equations, not the stale estimates"
        );
        assert!(objective.improvement.mean.unwrap().abs() < 1e-12);
    }

    /// Builds a metric with a native unit and a point-mass current estimate.
    fn measured(id: u64, name: &str, title: &str, unit: Unit, value: f64) -> Node {
        Node::new(
            EntityId::new(id),
            name,
            title,
            NodePayload::Metric(
                Metric::with_quantity(
                    QuantityDefinition::with_dimension(
                        name,
                        Some(unit),
                        None,
                        QuantitySupport::NonNegative,
                    )
                    .unwrap(),
                    Some(estimate::<QuantityValue>(0, value)),
                )
                .unwrap(),
            ),
        )
        .unwrap()
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
            EdgePayload::Changes(CausalEffect::proportional(
                estimate::<Elasticity>(0, 1.6),
                None,
                String::new(),
                vec![],
            )),
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
            EdgePayload::Contributes(CausalEffect::proportional(
                estimate::<Elasticity>(0, effect),
                None,
                String::new(),
                vec![],
            )),
        )
        .unwrap()
    }

    /// Reports a feedback loop and the gain that decides whether it settles.
    ///
    /// The fixture closes the factor/outcome pair into a circuit whose responses
    /// multiply to $0.2 \times 3.0 = 0.6$, so a deviation entering the loop keeps
    /// only 60% of itself each trip and dies out. Raising the return response to
    /// 6.0 makes the product 1.2, which grows without bound until the declared
    /// support clamps it — a projection that reports the bound rather than the
    /// intervention, which is exactly what the diagnostic exists to reveal.
    #[test]
    fn reports_a_feedback_loop_and_whether_it_settles() {
        let (scenario, nodes, mut edges, revision) = point_fixture(3);
        let returns = |effect: f64| {
            Edge::new(
                nodes[2].id,
                NodeKind::Outcome,
                nodes[1].id,
                NodeKind::Factor,
                EdgePayload::Contributes(CausalEffect::proportional(
                    estimate::<Elasticity>(0, effect),
                    None,
                    String::new(),
                    vec![],
                )),
            )
            .unwrap()
        };
        edges.push(returns(3.0));

        let damped =
            ScenarioAnalysis::compute(revision.clone(), &scenario, &nodes, &edges, None).unwrap();
        assert_eq!(damped.feedback_loops.len(), 1, "one circuit, reported once");
        let loop_ = &damped.feedback_loops[0];
        assert_eq!(
            loop_
                .states
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            [nodes[1].id, nodes[2].id].into_iter().collect(),
        );
        assert!((loop_.gain.unwrap() - 0.6).abs() < 1e-12);
        assert!(!loop_.is_amplifying() && loop_.settles());

        edges.pop();
        edges.push(returns(6.0));
        let amplifying =
            ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges, None).unwrap();
        let loop_ = &amplifying.feedback_loops[0];
        assert!((loop_.gain.unwrap() - 1.2).abs() < 1e-12);
        assert!(
            loop_.is_amplifying() && !loop_.settles(),
            "a gain above one must be flagged before the numbers are trusted"
        );
        assert!(
            amplifying.candidates[0].clamped_state_updates > 0,
            "the fixture must actually saturate, or the diagnostic proves nothing"
        );
    }

    /// Weighs a loop that runs through a node equation.
    ///
    /// An equation is arbitrary arithmetic, so no elasticity is authored for the
    /// edge feeding it. One is still measurable: nudge the parent about its
    /// baseline and read the relative change in the result. That is the same
    /// linearisation the gain already assumes for an authored response, so the
    /// two multiply together. Here the outcome equals its parent outright, which
    /// is an elasticity of exactly one, leaving the loop's gain to the authored
    /// 0.1 on the return hop.
    ///
    /// Instability stays absent: sampling multiplies authored distributions, and
    /// an equation supplies none.
    #[test]
    fn measures_an_equation_elasticity_by_differentiating_it() {
        let (scenario, mut nodes, mut edges, revision) = point_fixture(3);
        nodes[2].native_state = Some(nodes[2].native_state.clone().unwrap().with_relation(Some(
            StateRelation::new("feedback".to_owned(), Default::default()).unwrap(),
        )));
        edges.push(
            Edge::new(
                nodes[2].id,
                NodeKind::Outcome,
                nodes[1].id,
                NodeKind::Factor,
                EdgePayload::Contributes(CausalEffect::proportional(
                    estimate::<Elasticity>(0, 0.1),
                    None,
                    String::new(),
                    vec![],
                )),
            )
            .unwrap(),
        );

        let result = ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges, None).unwrap();
        assert_eq!(result.feedback_loops.len(), 1);
        let loop_ = &result.feedback_loops[0];
        assert!(
            (loop_.gain.unwrap() - 0.1).abs() < 1e-6,
            "the equation contributes an elasticity of one, got {:?}",
            loop_.gain,
        );
        assert!(loop_.settles() && !loop_.needs_review());
        assert_eq!(
            loop_.instability, None,
            "an equation supplies no distribution to sample"
        );

        let weights = &loop_.weights;
        assert_eq!(weights.len(), 2, "one weight per hop");
        let equation = weights
            .iter()
            .find(|weight| weight.destination == nodes[2].id)
            .unwrap();
        assert!((equation.response - 1.0).abs() < 1e-6);
        assert!(
            equation.contribution.abs() < 1e-6,
            "a response of one neither amplifies nor damps"
        );
        let authored = weights
            .iter()
            .find(|weight| weight.destination == nodes[1].id)
            .unwrap();
        assert!((authored.response - 0.1).abs() < 1e-12);
        assert!(
            (authored.contribution - 0.1_f64.ln()).abs() < 1e-12,
            "the damping hop carries the whole of the log gain"
        );
        assert!(
            (weights
                .iter()
                .map(|weight| weight.contribution)
                .sum::<f64>()
                - loop_.gain.unwrap().abs().ln())
            .abs()
                < 1e-6,
            "contributions must decompose the log gain exactly"
        );
    }

    /// Reports how often an uncertain loop fails to contract, not just on average.
    ///
    /// Both responses average 0.9, so the mean gain is 0.81 and the loop looks
    /// safe. They are uncertain enough that their product still exceeds one in a
    /// large minority of draws, and those are the draws where the projection
    /// reports its clamp rather than the plan. A point estimate cannot say that,
    /// which is the whole reason the sampled share is carried beside it.
    #[test]
    fn measures_how_often_an_uncertain_loop_fails_to_contract() {
        let uncertain = |spread: f64| {
            let (mut scenario, nodes, mut edges, revision) = point_fixture(3);
            let response = |id: u64| {
                let distribution = if spread > 0.0 {
                    Distribution::scaled_beta(1.0, 1.0, 0.9 - spread, 0.9 + spread).unwrap()
                } else {
                    Distribution::point(0.9).unwrap()
                };
                Estimate::<Elasticity>::new(EstimateId::new(id), distribution).unwrap()
            };
            let EdgePayload::Contributes(effect) = &mut edges[1].payload else {
                unreachable!()
            };
            effect.response = response(0);
            edges.push(
                Edge::new(
                    nodes[2].id,
                    NodeKind::Outcome,
                    nodes[1].id,
                    NodeKind::Factor,
                    EdgePayload::Contributes(CausalEffect::proportional(
                        response(1),
                        None,
                        String::new(),
                        vec![],
                    )),
                )
                .unwrap(),
            );
            scenario.draft.monte_carlo = MonteCarloConfig::new(31, 2, 2, 0.5, 0.0).unwrap();
            let result =
                ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges, None).unwrap();
            result.feedback_loops.into_iter().next().unwrap()
        };

        let certain = uncertain(0.0);
        assert!((certain.gain.unwrap() - 0.81).abs() < 1e-12);
        assert_eq!(
            certain.instability,
            Some(0.0),
            "a loop with no uncertainty cannot cross one"
        );
        assert!(!certain.needs_review());

        let wide = uncertain(0.6);
        assert!(
            (wide.gain.unwrap() - certain.gain.unwrap()).abs() < 0.01,
            "the two loops must be indistinguishable by mean gain, got {:?} and {:?}",
            certain.gain,
            wide.gain,
        );
        let share = wide.instability.unwrap();
        assert!(
            share > 0.15 && share < 0.45,
            "an uncertain loop must report how often it runs away, got {share}"
        );
        assert!(
            wide.settles(),
            "the mean still contracts, which is exactly what hides the risk"
        );
        assert!(
            wide.needs_review(),
            "a loop that runs away in a fifth of draws must not be dismissed"
        );
    }

    /// Distinguishes an objective that cannot move yet from one that never will.
    ///
    /// Every relationship adds a transport period, so the outcome three hops from
    /// the intervention cannot respond before period three. Reachability reports
    /// the same `true` whatever the horizon, which is why the period count is
    /// carried alongside it: a horizon of two returns a flat zero that means "not
    /// yet", and only this count separates that from a disconnected objective.
    #[test]
    fn counts_the_periods_an_effect_needs_to_arrive() {
        let (mut scenario, mut nodes, mut edges, revision) = point_fixture(2);
        let outcome = nodes.pop().unwrap();
        let relay = with_state(
            Node::new(
                EntityId::new(3),
                "relay",
                "Relay",
                NodePayload::Factor(Factor {
                    controllable: false,
                    evidence: vec![],
                }),
            )
            .unwrap(),
            0.5,
        );
        // Route the existing factor through a relay instead of straight to the
        // outcome, which lengthens the chain without changing its strength.
        edges.pop();
        edges.push(contributes(nodes[1].id, relay.id, 0.2));
        edges.push(contributes(relay.id, outcome.id, 0.2));
        nodes.extend([relay, outcome]);

        let short =
            ScenarioAnalysis::compute(revision.clone(), &scenario, &nodes, &edges, None).unwrap();
        let objective = &short.candidates[0].objectives[0];
        assert!(objective.reachable);
        assert_eq!(
            objective.periods_to_effect,
            Some(3),
            "one period into the factor, then one per relationship"
        );
        assert!(
            objective.periods_to_effect > Some(short.planning_horizon),
            "the fixture must outrun its horizon"
        );
        assert_eq!(objective.improvement.mean, Some(0.0));

        scenario.draft.planning_horizon = 4;
        let long = ScenarioAnalysis::compute(revision, &scenario, &nodes, &edges, None).unwrap();
        let objective = &long.candidates[0].objectives[0];
        assert_eq!(objective.periods_to_effect, Some(3));
        assert!(
            objective.improvement.mean.unwrap() > 0.0,
            "the same model must move once the horizon covers the delay"
        );
    }
}
