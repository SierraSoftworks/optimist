use optimist::domain::{
    BlockingEffect, CausalEffect, Distribution, Edge, EdgePayload, EntityId, Estimate, EstimateId,
    Factor, Intervention, Measurement, MeasurementPolarity, Metric, Node, NodeKind, NodePayload,
    NormalizedState, Observation, Outcome, OutcomeDirection, Requirement, SignedInfluence,
};
use proptest::prelude::*;

pub(crate) fn project_id() -> impl Strategy<Value = optimist::domain::ProjectId> {
    proptest::string::string_regex("[A-Za-z0-9_.]{1,32}")
        .expect("valid project ID expression")
        .prop_map(|value| optimist::domain::ProjectId::new(value).expect("generated project ID"))
}

pub(crate) fn entity_id() -> impl Strategy<Value = EntityId> {
    any::<u64>().prop_map(EntityId::new)
}

pub(crate) fn observation() -> impl Strategy<Value = Observation> {
    (
        any::<u64>(),
        any::<u64>(),
        -1_000_000_i32..1_000_000,
        proptest::option::of(0_u16..1_000),
        any::<bool>(),
    )
        .prop_map(|(id, revision, value, deviation, corrected)| Observation {
            id,
            revision,
            value: f64::from(value),
            unit: "ratio".to_owned(),
            observed_at: "2026-07-16T00:00:00Z".to_owned(),
            source: "property-test".to_owned(),
            measurement_standard_deviation: deviation.map(f64::from),
            supersedes: corrected.then(|| id.checked_sub(1)).flatten(),
        })
}

pub(crate) fn node() -> impl Strategy<Value = Node> {
    (
        entity_id(),
        proptest::string::string_regex("[a-z][a-z0-9_]{0,15}").expect("valid node name expression"),
        0_u8..4,
        any::<bool>(),
        0_u8..=4,
    )
        .prop_map(|(id, name, variant, flag, state)| {
            let state = f64::from(state) / 4.0;
            let payload = match variant {
                0 => NodePayload::Outcome(Outcome {
                    direction: if flag {
                        OutcomeDirection::Maximize
                    } else {
                        OutcomeDirection::Minimize
                    },
                    current: Some(normalized_estimate(state)),
                    desired: None,
                    evidence: Vec::new(),
                }),
                1 => NodePayload::Metric(
                    Metric::new("ratio", flag.then(|| "weekly".to_owned())).unwrap(),
                ),
                2 => NodePayload::Factor(Factor {
                    current: Some(normalized_estimate(state)),
                    desired: None,
                    controllable: flag,
                    evidence: Vec::new(),
                }),
                _ => NodePayload::Intervention(Intervention {
                    costs: Vec::new(),
                    duration: None,
                    probability_of_success: None,
                    acceptance_criteria: Vec::new(),
                }),
            };
            Node::new(id, name.clone(), format!("Title {name}"), payload)
                .expect("generated node is valid")
        })
}

pub(crate) type EndpointTuple = (EntityId, NodeKind, EntityId, NodeKind, EdgePayload);

pub(crate) fn valid_endpoints() -> impl Strategy<Value = EndpointTuple> {
    (any::<u64>(), 1_u16..=u16::MAX, 0_u8..8, -2_i8..=2).prop_map(
        |(source, offset, variant, effect)| {
            let source = EntityId::new(source);
            let destination = EntityId::new(source.value().wrapping_add(u64::from(offset)));
            let effect = f64::from(effect) / 2.0;
            match variant {
                0 => (
                    source,
                    NodeKind::Factor,
                    destination,
                    NodeKind::Outcome,
                    EdgePayload::Contributes(causal_effect(effect)),
                ),
                1 => (
                    source,
                    NodeKind::Metric,
                    destination,
                    NodeKind::Factor,
                    EdgePayload::Measures(Measurement {
                        polarity: MeasurementPolarity::HigherIsBetter,
                        calibration: None,
                        observations: Vec::new(),
                    }),
                ),
                2 => (
                    source,
                    NodeKind::Intervention,
                    destination,
                    NodeKind::Factor,
                    EdgePayload::Changes(causal_effect(effect)),
                ),
                3 => (
                    source,
                    NodeKind::Factor,
                    destination,
                    NodeKind::Intervention,
                    EdgePayload::Requires(Requirement {
                        hard: effect >= 0.0,
                        satisfaction_threshold: Some((effect + 1.0) / 2.0),
                    }),
                ),
                4 => (
                    source,
                    NodeKind::Factor,
                    destination,
                    NodeKind::Factor,
                    EdgePayload::PartOf,
                ),
                5 => (
                    source,
                    NodeKind::Factor,
                    destination,
                    NodeKind::Intervention,
                    EdgePayload::Blocks(BlockingEffect {
                        degree: signed_estimate(effect),
                    }),
                ),
                6 => (
                    source,
                    NodeKind::Intervention,
                    destination,
                    NodeKind::Intervention,
                    EdgePayload::ConflictsWith,
                ),
                _ => (
                    source,
                    NodeKind::Intervention,
                    destination,
                    NodeKind::Intervention,
                    EdgePayload::SynergizesWith,
                ),
            }
        },
    )
}

pub(crate) fn edge() -> impl Strategy<Value = Edge> {
    valid_endpoints().prop_map(
        |(source, source_kind, destination, destination_kind, payload)| {
            Edge::new(source, source_kind, destination, destination_kind, payload)
                .expect("generated endpoints are valid")
        },
    )
}

fn normalized_estimate(value: f64) -> Estimate<NormalizedState> {
    Estimate::new(
        EstimateId::new(0),
        Distribution::point(value).expect("generated finite point distribution"),
    )
    .expect("generated normalized estimate")
}

fn signed_estimate(value: f64) -> Estimate<SignedInfluence> {
    Estimate::new(
        EstimateId::new(0),
        Distribution::point(value).expect("generated finite point distribution"),
    )
    .expect("generated signed estimate")
}

fn causal_effect(value: f64) -> CausalEffect {
    CausalEffect {
        effect: signed_estimate(value),
        lag: None,
        mechanism: String::new(),
        evidence: Vec::new(),
    }
}
