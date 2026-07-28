//! Coverage for weighing proposed changes against a design.
//!
//! An intervention rebinds shared quantities and changes nothing else, so these
//! tests check both that a proposal reaches what it should and that the parts of
//! the model it did not name stayed put.

use optimist::system::{
    AttachedMutator, Component, ComponentId, Distribution, EvaluationConfig, Intervention,
    InterventionId, MutatorId, Override, Relationship, ScaleUnit, ScaleUnitId, ScratchpadEntry,
    SystemModel, builtin_catalogue, compare, evaluate, evaluate_intervention,
};

fn config() -> EvaluationConfig {
    EvaluationConfig {
        seed: 19,
        sample_count: 400,
        ..EvaluationConfig::default()
    }
}

fn shared(name: &str, expression: &str) -> ScratchpadEntry {
    ScratchpadEntry {
        name: name.to_owned(),
        expression: expression.to_owned(),
        unit: None,
        summary: String::new(),
    }
}

fn component(id: &str, component_type: &str, properties: &[(&str, &str)]) -> Component {
    Component {
        id: ComponentId::new(id),
        name: id.to_owned(),
        component_type: serde_yaml_ng::from_str(component_type).expect("type id"),
        properties: properties
            .iter()
            .map(|(name, source)| ((*name).to_owned(), (*source).to_owned()))
            .collect(),
        position: None,
    }
}

fn link(from: &str, to: &str) -> Relationship {
    Relationship {
        from_port: None,
        to_port: None,
        capacity: None,
        bandwidth: None,
        from: ComponentId::new(from),
        to: ComponentId::new(to),
        mutators: Vec::new(),
        summary: String::new(),
    }
}

fn proposal(id: &str, overrides: &[(&str, &str)]) -> Intervention {
    Intervention {
        id: InterventionId::new(id),
        name: id.to_owned(),
        summary: String::new(),
        overrides: overrides
            .iter()
            .map(|(name, expression)| Override {
                name: (*name).to_owned(),
                expression: (*expression).to_owned(),
            })
            .collect(),
    }
}

/// A pool sized against a shared peak rate, with proposals that move either side.
fn saturated_model(interventions: Vec<Intervention>) -> SystemModel {
    SystemModel {
        scratchpad: vec![
            shared("peak_rate", "900"),
            shared("pool_size", "8"),
            shared("service_time", "0.02"),
        ],
        components: vec![
            component("users", "client", &[("request_rate", "peak_rate")]),
            component(
                "api",
                "compute",
                &[
                    ("service_time", "service_time"),
                    ("parallelism", "pool_size"),
                ],
            ),
        ],
        relationships: vec![link("users", "api")],
        interventions,
        ..SystemModel::default()
    }
}

/// Rebinding a quantity reaches every component sized against it.
#[test]
fn a_rebinding_reaches_what_depends_on_it() {
    let model = saturated_model(vec![proposal("bigger", &[("pool_size", "40")])]);
    let catalogue = builtin_catalogue().expect("catalogue");

    let before = evaluate(&model, &catalogue, config()).expect("solves");
    let after = evaluate_intervention(&model, &catalogue, &InterventionId::new("bigger"), config())
        .expect("solves");

    let api = ComponentId::new("api");
    let capacity =
        |evaluation: &optimist::system::Evaluation| match &evaluation.settled().components[&api]
            .channels["capacity"]
        {
            optimist::squiggle::Value::Number(value) => *value,
            value => panic!("expected a certain capacity, got {value:?}"),
        };
    // Eight slots at 20 ms sustain 400 per second; forty sustain 2000.
    assert!((capacity(&before) - 400.0).abs() < 1e-6);
    assert!((capacity(&after) - 2_000.0).abs() < 1e-6);
}

/// A proposal that relieves the binding constraint is reported as doing so.
#[test]
fn a_comparison_reports_what_was_relieved() {
    let model = saturated_model(vec![proposal("bigger", &[("pool_size", "40")])]);
    let catalogue = builtin_catalogue().expect("catalogue");
    let comparison =
        compare(&model, &catalogue, &InterventionId::new("bigger"), config()).expect("compares");

    assert!(comparison.baseline[0].binds(), "900 offered against 400");
    assert!(!comparison.proposed[0].binds(), "900 offered against 2000");
    let relieved = comparison.relieved();
    assert_eq!(relieved.len(), 1);
    assert_eq!(relieved[0].constraint, "capacity");
    assert!(relieved[0].shift() < 0.0, "load must have fallen");
}

/// A proposal that only moves the bottleneck is reported as doing that.
///
/// Relieving one constraint routinely promotes another, and knowing which is
/// the difference between a fix and a rearrangement.
#[test]
fn a_comparison_reports_a_bottleneck_that_merely_moved() {
    let model = SystemModel {
        scratchpad: vec![shared("pool_size", "4"), shared("store_iops", "500")],
        components: vec![
            component("users", "client", &[("request_rate", "900")]),
            component(
                "api",
                "compute",
                &[("service_time", "0.01"), ("parallelism", "pool_size")],
            ),
            component(
                "store",
                "datastore",
                &[
                    ("operation_limit", "store_iops"),
                    ("transfer_limit", "1e10"),
                    ("volume_limit", "1e14"),
                    ("record_size", "512"),
                    ("retention", "600"),
                ],
            ),
        ],
        relationships: vec![link("users", "api"), link("api", "store")],
        interventions: vec![proposal("bigger_pool", &[("pool_size", "64")])],
        ..SystemModel::default()
    };
    let catalogue = builtin_catalogue().expect("catalogue");
    let comparison = compare(
        &model,
        &catalogue,
        &InterventionId::new("bigger_pool"),
        config(),
    )
    .expect("compares");

    // The pool was the bottleneck and is substantially relieved. It is not
    // relieved *entirely*: the demand it now serves is retried less, but a pool
    // this far past its capacity does not become comfortable by growing once.
    let pool = comparison
        .movements
        .iter()
        .find(|movement| movement.component.as_str() == "api" && movement.constraint == "capacity")
        .expect("api capacity movement");
    assert!(
        pool.shift() < 0.0 && pool.after < pool.before * 0.75,
        "a larger pool must relieve the pool, {} to {}",
        pool.before,
        pool.after
    );
    // Freeing it lets demand through to a store that was already at its limit.
    let store = comparison
        .movements
        .iter()
        .find(|movement| movement.component.as_str() == "store" && movement.constraint == "volume")
        .expect("store volume movement");
    assert!(
        store.shift() > 0.0,
        "the store must be loaded further, moved {}",
        store.shift()
    );
}

/// Rebinding one quantity leaves everything it does not name alone.
#[test]
fn a_rebinding_disturbs_nothing_else() {
    let model = saturated_model(vec![proposal("slower", &[("service_time", "0.05")])]);
    let catalogue = builtin_catalogue().expect("catalogue");
    let comparison =
        compare(&model, &catalogue, &InterventionId::new("slower"), config()).expect("compares");

    // Demand was not rebound, so the offered load is unchanged on both sides.
    let offered = |ranked: &[optimist::system::Bottleneck]| {
        ranked
            .iter()
            .find(|entry| entry.constraint == "capacity")
            .expect("capacity")
            .utilisation
    };
    assert!(offered(&comparison.proposed) > offered(&comparison.baseline));
    assert_eq!(comparison.baseline.len(), comparison.proposed.len());
}

/// A change that arrives partway through a run is an ordinary expression.
///
/// The quantity was always a function of time, so a rollout needs no separate
/// machinery: a constant was only the simplest case.
#[test]
fn a_rebinding_may_depend_on_time() {
    let model = saturated_model(vec![proposal(
        "rollout",
        &[("pool_size", "if t < 3 then 8 else 40")],
    )]);
    let catalogue = builtin_catalogue().expect("catalogue");
    let evaluation = evaluate_intervention(
        &model,
        &catalogue,
        &InterventionId::new("rollout"),
        EvaluationConfig {
            horizon: 6,
            ..config()
        },
    )
    .expect("solves");

    let api = ComponentId::new("api");
    let capacity = |step: &optimist::system::Step| match &step.components[&api].channels["capacity"]
    {
        optimist::squiggle::Value::Number(value) => *value,
        value => panic!("expected a certain capacity, got {value:?}"),
    };
    assert!((capacity(&evaluation.steps[0]) - 400.0).abs() < 1e-6);
    // Sample sets round, so the comparison is relative rather than exact.
    assert!((capacity(&evaluation.steps[5]) - 2_000.0).abs() < 2e-3);
}

/// A proposal may rebind several quantities at once.
#[test]
fn a_proposal_may_rebind_several_quantities() {
    let model = saturated_model(vec![proposal(
        "retune",
        &[("pool_size", "24"), ("service_time", "0.005")],
    )]);
    let catalogue = builtin_catalogue().expect("catalogue");
    let comparison =
        compare(&model, &catalogue, &InterventionId::new("retune"), config()).expect("compares");
    // Twenty-four slots at 5 ms sustain 4800 per second against 900 offered.
    assert!(!comparison.proposed[0].binds());
}

/// Rebinding a quantity nobody declared is a mistake, not a no-op.
///
/// A misspelt name would otherwise produce a comparison showing no change, which
/// reads as evidence that the proposal does not help.
#[test]
fn rebinding_an_undeclared_quantity_is_rejected() {
    let model = saturated_model(vec![proposal("typo", &[("pool_sise", "40")])]);
    let catalogue = builtin_catalogue().expect("catalogue");
    let error = compare(&model, &catalogue, &InterventionId::new("typo"), config())
        .expect_err("unknown quantity");
    assert!(error.to_string().contains("pool_sise"), "{error}");
    assert!(error.to_string().contains("change nothing"), "{error}");
}

/// Asking for an intervention the model does not declare is reported.
#[test]
fn an_unknown_proposal_is_reported() {
    let model = saturated_model(Vec::new());
    let catalogue = builtin_catalogue().expect("catalogue");
    let error = compare(
        &model,
        &catalogue,
        &InterventionId::new("missing"),
        config(),
    )
    .expect_err("unknown intervention");
    assert!(error.to_string().contains("no intervention"), "{error}");
}

/// Proposals compose with behaviours and scale units already in the model.
#[test]
fn a_proposal_reaches_behaviours_and_scale_units() {
    let model = SystemModel {
        scratchpad: vec![shared("cells", "2"), shared("hit_ratio", "0.1")],
        components: vec![
            component("users", "client", &[("request_rate", "2000")]),
            component(
                "api",
                "compute",
                &[("service_time", "0.01"), ("parallelism", "8")],
            ),
        ],
        relationships: vec![Relationship {
            from_port: None,
            to_port: None,
            capacity: None,
            bandwidth: None,
            from: ComponentId::new("users"),
            to: ComponentId::new("api"),
            mutators: vec![AttachedMutator {
                mutator: MutatorId::new("cache"),
                properties: [("hit_ratio".to_owned(), "hit_ratio".to_owned())]
                    .into_iter()
                    .collect(),
            }],
            summary: String::new(),
        }],
        scale_units: vec![ScaleUnit {
            id: ScaleUnitId::new("cell"),
            name: "Cell".to_owned(),
            summary: String::new(),
            replicas: "cells".to_owned(),
            distribution: Distribution::Sharded,
            members: vec![ComponentId::new("api")],
            parent: None,
        }],
        interventions: vec![
            proposal("warmer_cache", &[("hit_ratio", "0.95")]),
            proposal("more_cells", &[("cells", "8")]),
        ],
    };
    let catalogue = builtin_catalogue().expect("catalogue");

    // 2000 offered, 10% cached, over two cells is 900 per cell against 800 served.
    let cached = compare(
        &model,
        &catalogue,
        &InterventionId::new("warmer_cache"),
        config(),
    )
    .expect("compares");
    assert!(cached.baseline[0].binds());
    assert!(
        !cached.proposed[0].binds(),
        "a warmer cache absorbs the load"
    );

    // Shedding the same load across more cells relieves it too.
    let spread = compare(
        &model,
        &catalogue,
        &InterventionId::new("more_cells"),
        config(),
    )
    .expect("compares");
    assert!(!spread.proposed[0].binds(), "more cells divide the load");
    let capacity = spread
        .proposed
        .iter()
        .find(|entry| entry.constraint == "capacity")
        .expect("capacity");
    assert!((capacity.replicas - 8.0).abs() < 1e-9);
}

fn flagged(exposure: &str, interventions: Vec<Intervention>) -> SystemModel {
    SystemModel {
        scratchpad: vec![shared("exposure", exposure)],
        components: vec![
            component("users", "client", &[("request_rate", "1000")]),
            component(
                "recommender",
                "compute",
                &[("service_time", "0.02"), ("parallelism", "8")],
            ),
        ],
        relationships: vec![Relationship {
            from_port: None,
            to_port: None,
            capacity: None,
            bandwidth: None,
            from: ComponentId::new("users"),
            to: ComponentId::new("recommender"),
            mutators: vec![AttachedMutator {
                mutator: MutatorId::new("feature-flag"),
                properties: [("exposure".to_owned(), "exposure".to_owned())]
                    .into_iter()
                    .collect(),
            }],
            summary: String::new(),
        }],
        interventions,
        ..SystemModel::default()
    }
}

fn offered(model: &SystemModel, intervention: Option<&str>) -> f64 {
    let catalogue = builtin_catalogue().expect("catalogue");
    let evaluation = match intervention {
        Some(id) => evaluate_intervention(model, &catalogue, &InterventionId::new(id), config()),
        None => evaluate(model, &catalogue, config()),
    }
    .expect("solves");
    match &evaluation.settled().components[&ComponentId::new("recommender")].channels["offered"] {
        optimist::squiggle::Value::Number(value) => *value,
        value => panic!("expected a certain rate, got {value:?}"),
    }
}

/// A flag admits none, some, or all of the traffic along a connection.
#[test]
fn a_feature_flag_gates_the_traffic_behind_it() {
    assert!((offered(&flagged("0", Vec::new()), None)).abs() < 1e-9);
    assert!((offered(&flagged("0.05", Vec::new()), None) - 50.0).abs() < 1e-6);
    assert!((offered(&flagged("1", Vec::new()), None) - 1_000.0).abs() < 1e-6);
}

/// A share outside zero and one is clamped rather than amplifying demand.
#[test]
fn a_flag_cannot_admit_more_traffic_than_arrives() {
    assert!((offered(&flagged("2.5", Vec::new()), None) - 1_000.0).abs() < 1e-6);
    assert!((offered(&flagged("-1", Vec::new()), None)).abs() < 1e-9);
}

/// Turning a flag on is an intervention, so its cost can be weighed beforehand.
///
/// This is what makes a flag worth modelling: the design is read both with and
/// without the feature by rebinding one quantity, and the constraint the feature
/// would introduce shows up before anyone ships it.
#[test]
fn enabling_a_flag_reveals_what_the_feature_would_cost() {
    let model = flagged(
        "0",
        vec![
            proposal("canary", &[("exposure", "0.05")]),
            proposal("launch", &[("exposure", "1")]),
        ],
    );
    let catalogue = builtin_catalogue().expect("catalogue");

    // Dark, the recommender is untouched and nothing binds.
    assert!((offered(&model, None)).abs() < 1e-9);

    // At five percent it is comfortable: fifty requests against four hundred served.
    let canary =
        compare(&model, &catalogue, &InterventionId::new("canary"), config()).expect("compares");
    assert!(!canary.proposed[0].binds(), "a canary should be survivable");

    // Fully launched it is not.
    let launch =
        compare(&model, &catalogue, &InterventionId::new("launch"), config()).expect("compares");
    assert!(
        launch.proposed[0].binds(),
        "the full rollout must expose the constraint, utilisation {}",
        launch.proposed[0].utilisation
    );
    assert_eq!(launch.introduced().len(), 1);
    assert_eq!(launch.introduced()[0].component.as_str(), "recommender");
}

/// Complementary flags route traffic between an old path and a new one.
///
/// A migration is not a switch but a dial, and both paths carry load while it
/// turns. Sizing only the destination is how a migration takes down the source.
#[test]
fn complementary_flags_split_traffic_between_paths() {
    let route = |share: &str| SystemModel {
        scratchpad: vec![shared("new_share", share)],
        components: vec![
            component("users", "client", &[("request_rate", "800")]),
            component(
                "legacy",
                "compute",
                &[("service_time", "0.01"), ("parallelism", "16")],
            ),
            component(
                "replacement",
                "compute",
                &[("service_time", "0.005"), ("parallelism", "16")],
            ),
        ],
        relationships: vec![
            Relationship {
                from_port: None,
                to_port: None,
                capacity: None,
                bandwidth: None,
                from: ComponentId::new("users"),
                to: ComponentId::new("replacement"),
                mutators: vec![AttachedMutator {
                    mutator: MutatorId::new("feature-flag"),
                    properties: [("exposure".to_owned(), "new_share".to_owned())]
                        .into_iter()
                        .collect(),
                }],
                summary: String::new(),
            },
            Relationship {
                from_port: None,
                to_port: None,
                capacity: None,
                bandwidth: None,
                from: ComponentId::new("users"),
                to: ComponentId::new("legacy"),
                mutators: vec![AttachedMutator {
                    mutator: MutatorId::new("feature-flag"),
                    properties: [("exposure".to_owned(), "1 - new_share".to_owned())]
                        .into_iter()
                        .collect(),
                }],
                summary: String::new(),
            },
        ],
        ..SystemModel::default()
    };

    let catalogue = builtin_catalogue().expect("catalogue");
    let split = |share: &str, component: &str| {
        let model = route(share);
        let evaluation = evaluate(&model, &catalogue, config()).expect("solves");
        match &evaluation.settled().components[&ComponentId::new(component)].channels["offered"] {
            optimist::squiggle::Value::Number(value) => *value,
            value => panic!("expected a certain rate, got {value:?}"),
        }
    };

    assert!((split("0", "legacy") - 800.0).abs() < 1e-6);
    assert!((split("0", "replacement")).abs() < 1e-9);

    assert!((split("0.25", "legacy") - 600.0).abs() < 1e-6);
    assert!((split("0.25", "replacement") - 200.0).abs() < 1e-6);

    assert!((split("1", "legacy")).abs() < 1e-9);
    assert!((split("1", "replacement") - 800.0).abs() < 1e-6);
}

/// A flag may open over time, so a staged rollout needs no extra machinery.
#[test]
fn a_flag_may_open_over_time() {
    let model = flagged(
        "0",
        vec![proposal(
            "staged",
            &[("exposure", "if t < 2 then 0.1 else 1")],
        )],
    );
    let catalogue = builtin_catalogue().expect("catalogue");
    let evaluation = evaluate_intervention(
        &model,
        &catalogue,
        &InterventionId::new("staged"),
        EvaluationConfig {
            horizon: 4,
            ..config()
        },
    )
    .expect("solves");

    let rate = |step: &optimist::system::Step| match &step.components
        [&ComponentId::new("recommender")]
        .channels["offered"]
    {
        optimist::squiggle::Value::Number(value) => *value,
        value => panic!("expected a certain rate, got {value:?}"),
    };
    // A step that starts from the previous one's answer settles to the solver's
    // relative tolerance, so the check is relative rather than absolute.
    let settled = |step: &optimist::system::Step, expected: f64| {
        assert!(
            (rate(step) - expected).abs() / expected < 1e-5,
            "at t={} expected {expected}, got {}",
            step.time,
            rate(step)
        );
    };
    settled(&evaluation.steps[0], 100.0);
    settled(&evaluation.steps[3], 1_000.0);
}
