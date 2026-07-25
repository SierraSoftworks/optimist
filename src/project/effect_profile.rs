use crate::{
    command::{
        CommandOutcome, EffectAftereffectInput, EffectProfileInput, EffectReleaseInput,
        SetEffectProfile, UpdateCausalEffect,
    },
    domain::{
        Duration, Edge, EdgeId, EdgePayload, EffectAftereffect, EffectProfile, EffectRelease,
        EffectTransience, Elasticity, Estimate, EstimateId, SquiggleEstimateDefinition, Unit,
    },
    store::{GraphRepository, RepositoryError},
};

use super::{AggregateUpdateError, EstimateCommandError, ProjectError, catalog::ProjectEntry};

/// Replaces one causal relationship's explanation and evidence.
pub(super) fn update(
    entry: &mut ProjectEntry,
    command: UpdateCausalEffect,
) -> Result<CommandOutcome, ProjectError> {
    let mut edge = guarded(entry, &command.edge, command.expected_revision)?;
    let next_revision = next_revision(&edge, &command.edge)?;
    let effect = match &mut edge.payload {
        EdgePayload::Contributes(effect) | EdgePayload::Changes(effect) => effect,
        _ => return Err(ProjectError::NotCausalEdge(command.edge)),
    };
    effect.mechanism = command.mechanism;
    effect.evidence = command.evidence;
    edge.revision = next_revision;
    entry.repository.update_edge(edge.clone())?;
    Ok(CommandOutcome::CausalEffectUpdated(edge))
}

pub(super) fn set(
    entry: &mut ProjectEntry,
    command: SetEffectProfile,
) -> Result<CommandOutcome, ProjectError> {
    let mut edge = guarded(entry, &command.edge, command.expected_revision)?;
    let EdgePayload::Changes(effect) = &edge.payload else {
        return Err(ProjectError::NotInterventionEffectEdge(command.edge));
    };
    let mut ids = Allocator::after(effect);
    let transience = command
        .profile
        .map(|input| build(*input, &mut ids))
        .transpose()?;
    let next_revision = next_revision(&edge, &command.edge)?;
    let EdgePayload::Changes(effect) = &mut edge.payload else {
        unreachable!("the payload was matched as an intervention effect above")
    };
    *effect = effect.clone().with_transience(transience);
    edge.revision = next_revision;
    entry.repository.update_edge(edge.clone())?;
    Ok(CommandOutcome::EffectProfileSet(edge))
}

fn guarded(
    entry: &mut ProjectEntry,
    id: &EdgeId,
    expected_revision: u64,
) -> Result<Edge, ProjectError> {
    let edge = entry
        .repository
        .get_edge(id)?
        .ok_or_else(|| RepositoryError::MissingEdge(id.to_string()))?;
    if edge.revision != expected_revision {
        return Err(AggregateUpdateError::EdgeRevisionConflict {
            id: id.clone(),
            expected: expected_revision,
            current: edge.revision,
        }
        .into());
    }
    Ok(edge)
}

fn next_revision(edge: &Edge, id: &EdgeId) -> Result<u64, ProjectError> {
    edge.revision
        .checked_add(1)
        .ok_or_else(|| ProjectError::EdgeRevisionSpaceExhausted(id.clone()))
}

fn build(input: EffectProfileInput, ids: &mut Allocator) -> Result<EffectTransience, ProjectError> {
    let ramp = input.ramp.map(|value| periods(value, ids)).transpose()?;
    let hold = input.hold.map(|value| periods(value, ids)).transpose()?;
    let release = release(input.release, ids)?;
    let Some(aftereffect) = input.aftereffect else {
        let profile = EffectProfile::new(ramp, hold, release, None)?;
        return EffectTransience::new(profile, None).map_err(ProjectError::from);
    };
    let EffectAftereffectInput {
        magnitude,
        hold: rebound_hold,
        release: rebound_release,
    } = aftereffect;
    let rebound =
        Estimate::<Elasticity>::from_squiggle(ids.next(), magnitude, &Unit::dimensionless())
            .map_err(EstimateCommandError::from)?;
    let aftereffect = EffectAftereffect {
        hold: rebound_hold.map(|value| periods(value, ids)).transpose()?,
        release: self::release(rebound_release, ids)?,
    };
    let profile = EffectProfile::new(ramp, hold, release, Some(aftereffect))?;
    EffectTransience::new(profile, Some(rebound)).map_err(ProjectError::from)
}

fn release(input: EffectReleaseInput, ids: &mut Allocator) -> Result<EffectRelease, ProjectError> {
    Ok(match input {
        EffectReleaseInput::Immediate => EffectRelease::Immediate,
        EffectReleaseInput::Linear { over } => EffectRelease::Linear {
            over: periods(over, ids)?,
        },
        EffectReleaseInput::Exponential { half_life } => EffectRelease::Exponential {
            half_life: periods(half_life, ids)?,
        },
    })
}

fn periods(
    definition: SquiggleEstimateDefinition,
    ids: &mut Allocator,
) -> Result<Estimate<Duration>, ProjectError> {
    let unit = Unit::base("duration").expect("duration is a valid base unit");
    Estimate::<Duration>::from_squiggle(ids.next(), definition, &unit)
        .map_err(EstimateCommandError::from)
        .map_err(ProjectError::from)
}

/// Issues profile-owned estimate IDs above every ID already used by the edge.
///
/// Profile estimates are replaced as one document rather than addressed
/// individually, but they still share the edge's aggregate-local ID space, so
/// they must never collide with the response or lag estimates beside them.
struct Allocator(u64);

impl Allocator {
    fn after(effect: &crate::domain::CausalEffect) -> Self {
        let response = effect.response.id.value();
        let lag = effect
            .lag
            .as_ref()
            .map_or(0, |estimate| estimate.id.value());
        Self(response.max(lag).saturating_add(1))
    }

    fn next(&mut self) -> EstimateId {
        let id = EstimateId::new(self.0);
        self.0 = self.0.saturating_add(1);
        id
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        command::{
            CommandOutcome, CommandRequest, CreateEdge, CreateNode, EffectAftereffectInput,
            EffectProfileInput, EffectReleaseInput, GraphCommand, SetEffectProfile,
            SetNodeQuantityState, UpdateCausalEffect,
        },
        domain::{
            CausalEffect, EdgeId, EdgeKind, EdgePayload, Elasticity, EntityId, Estimate,
            EstimateId, Factor, Intervention, NodePayload, QuantityDefinition, QuantitySupport,
            SquiggleEstimateDefinition, Unit,
        },
        project::{ProjectCatalog, ProjectError},
    };

    fn periods(source: &str) -> SquiggleEstimateDefinition {
        SquiggleEstimateDefinition {
            source: source.to_owned(),
            seed: 42,
            sample_count: 256,
            target_unit: Unit::base("duration").unwrap(),
        }
    }

    fn ratio() -> Unit {
        Unit::base("ratio").unwrap()
    }

    /// Builds an intervention whose `changes` edge targets a `ratio` factor.
    fn fixture() -> (ProjectCatalog, crate::domain::ProjectId, EdgeId) {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Transient".to_owned()).unwrap();
        let mut revision = 0;
        let mut execute = |catalog: &mut ProjectCatalog, command| {
            let result = catalog
                .execute(&project.id, CommandRequest::new(revision, command))
                .unwrap();
            revision = result.project_revision;
            result
        };
        execute(
            &mut catalog,
            GraphCommand::CreateNode(CreateNode {
                name: "freeze".to_owned(),
                title: "Freeze".to_owned(),
                payload: NodePayload::Intervention(Intervention {
                    costs: vec![],
                    duration: None,
                    probability_of_success: None,
                    acceptance_criteria: vec![],
                }),
            }),
        );
        execute(
            &mut catalog,
            GraphCommand::CreateNode(CreateNode {
                name: "change_rate".to_owned(),
                title: "Change rate".to_owned(),
                payload: NodePayload::Factor(Factor {
                    controllable: false,
                    evidence: vec![],
                }),
            }),
        );
        execute(
            &mut catalog,
            GraphCommand::SetNodeQuantityState(SetNodeQuantityState {
                node: EntityId::new(1),
                expected_revision: 0,
                quantity: QuantityDefinition::with_dimension(
                    "ratio",
                    Some(ratio()),
                    None,
                    QuantitySupport::NonNegative,
                )
                .unwrap(),
            }),
        );
        execute(
            &mut catalog,
            GraphCommand::CreateEdge(CreateEdge {
                source: EntityId::new(0),
                destination: EntityId::new(1),
                payload: EdgePayload::Changes(CausalEffect::proportional(
                    Estimate::<Elasticity>::from_squiggle(
                        EstimateId::new(0),
                        SquiggleEstimateDefinition {
                            source: "pointMass(0.1)".to_owned(),
                            seed: 42,
                            sample_count: 256,
                            target_unit: Unit::dimensionless(),
                        },
                        &Unit::dimensionless(),
                    )
                    .unwrap(),
                    None,
                    String::new(),
                    vec![],
                )),
            }),
        );
        let edge = EdgeId {
            source: EntityId::new(0),
            kind: EdgeKind::Changes,
            destination: EntityId::new(1),
        };
        (catalog, project.id, edge)
    }

    fn profile() -> Box<EffectProfileInput> {
        Box::new(EffectProfileInput {
            ramp: None,
            hold: Some(periods("pointMass(2)")),
            release: EffectReleaseInput::Immediate,
            aftereffect: Some(EffectAftereffectInput {
                magnitude: SquiggleEstimateDefinition {
                    source: "pointMass(1.25)".to_owned(),
                    seed: 42,
                    sample_count: 256,
                    target_unit: Unit::dimensionless(),
                },
                hold: Some(periods("pointMass(1)")),
                release: EffectReleaseInput::Immediate,
            }),
        })
    }

    #[test]
    fn rewrites_the_anchor_and_explanation_without_disturbing_the_profile() {
        let (mut catalog, project, edge) = fixture();
        catalog
            .execute(
                &project,
                CommandRequest::new(
                    4,
                    GraphCommand::SetEffectProfile(SetEffectProfile {
                        edge: edge.clone(),
                        expected_revision: 0,
                        profile: Some(profile()),
                    }),
                ),
            )
            .unwrap();
        let result = catalog
            .execute(
                &project,
                CommandRequest::new(
                    5,
                    GraphCommand::UpdateCausalEffect(UpdateCausalEffect {
                        edge,
                        expected_revision: 1,
                        mechanism: "Freezing changes suppresses the defect inflow.".to_owned(),
                        evidence: vec!["2026-Q2 freeze retrospective".to_owned()],
                    }),
                ),
            )
            .unwrap();
        let CommandOutcome::CausalEffectUpdated(stored) = result.outcome else {
            panic!("expected an updated causal edge")
        };
        let EdgePayload::Changes(effect) = &stored.payload else {
            unreachable!()
        };
        assert_eq!(
            effect.mechanism,
            "Freezing changes suppresses the defect inflow."
        );
        assert_eq!(
            effect.evidence,
            vec!["2026-Q2 freeze retrospective".to_owned()]
        );
        assert!(
            effect.transience.is_some(),
            "editing the claim must not silently drop the temporal shape"
        );
    }

    #[test]
    fn rejects_a_claim_edit_on_a_relationship_without_a_response() {
        let (mut catalog, project, _) = fixture();
        let measures = EdgeId {
            source: EntityId::new(1),
            kind: EdgeKind::Measures,
            destination: EntityId::new(1),
        };
        let invalid = catalog.execute(
            &project,
            CommandRequest::new(
                4,
                GraphCommand::UpdateCausalEffect(UpdateCausalEffect {
                    edge: measures,
                    expected_revision: 0,
                    mechanism: String::new(),
                    evidence: vec![],
                }),
            ),
        );
        assert!(invalid.is_err());
    }

    #[test]
    fn shapes_an_intervention_effect_and_allocates_distinct_estimate_ids() {
        let (mut catalog, project, edge) = fixture();
        let result = catalog
            .execute(
                &project,
                CommandRequest::new(
                    4,
                    GraphCommand::SetEffectProfile(SetEffectProfile {
                        edge: edge.clone(),
                        expected_revision: 0,
                        profile: Some(profile()),
                    }),
                ),
            )
            .unwrap();
        let CommandOutcome::EffectProfileSet(stored) = result.outcome else {
            panic!("expected a shaped intervention edge")
        };
        assert_eq!(stored.revision, 1);
        let EdgePayload::Changes(effect) = &stored.payload else {
            unreachable!()
        };
        let transience = effect.transience.as_ref().unwrap();
        assert!(!transience.profile.is_persistent());
        assert!(transience.rebound.is_some());
        let response = effect.response.id;
        let hold = transience.profile.hold.as_ref().unwrap().id;
        let rebound = transience.rebound.as_ref().unwrap().id;
        assert_ne!(response, hold);
        assert_ne!(hold, rebound);
        assert_ne!(response, rebound);
    }

    #[test]
    fn restores_a_permanent_effect_and_drops_its_rebound() {
        let (mut catalog, project, edge) = fixture();
        catalog
            .execute(
                &project,
                CommandRequest::new(
                    4,
                    GraphCommand::SetEffectProfile(SetEffectProfile {
                        edge: edge.clone(),
                        expected_revision: 0,
                        profile: Some(profile()),
                    }),
                ),
            )
            .unwrap();
        let result = catalog
            .execute(
                &project,
                CommandRequest::new(
                    5,
                    GraphCommand::SetEffectProfile(SetEffectProfile {
                        edge,
                        expected_revision: 1,
                        profile: None,
                    }),
                ),
            )
            .unwrap();
        let CommandOutcome::EffectProfileSet(stored) = result.outcome else {
            panic!("expected a restored intervention edge")
        };
        let EdgePayload::Changes(effect) = &stored.payload else {
            unreachable!()
        };
        assert!(effect.transience.is_none());
    }

    #[test]
    fn rejects_a_release_form_without_a_hold_window() {
        let (mut catalog, project, edge) = fixture();
        let invalid = catalog.execute(
            &project,
            CommandRequest::new(
                4,
                GraphCommand::SetEffectProfile(SetEffectProfile {
                    edge,
                    expected_revision: 0,
                    profile: Some(Box::new(EffectProfileInput {
                        ramp: None,
                        hold: None,
                        release: EffectReleaseInput::Linear {
                            over: periods("pointMass(2)"),
                        },
                        aftereffect: None,
                    })),
                }),
            ),
        );
        assert!(matches!(invalid, Err(ProjectError::EffectProfile(_))));
    }

    #[test]
    fn rejects_a_profile_on_a_relationship_which_is_always_in_effect() {
        let (mut catalog, project, _) = fixture();
        let contributes = EdgeId {
            source: EntityId::new(1),
            kind: EdgeKind::Contributes,
            destination: EntityId::new(1),
        };
        let missing = catalog.execute(
            &project,
            CommandRequest::new(
                4,
                GraphCommand::SetEffectProfile(SetEffectProfile {
                    edge: contributes,
                    expected_revision: 0,
                    profile: Some(profile()),
                }),
            ),
        );
        assert!(missing.is_err());
    }
}
