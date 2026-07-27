//! End-to-end coverage for solving a system model.
//!
//! These tests build models the way an engineer would, out of catalogue types
//! wired together, and check that the solved quantities obey the laws the
//! manifests claim to apply.

use std::collections::BTreeMap;

use optimist::squiggle::Value;
use optimist::system::{
    Bottleneck, Component, ComponentId, EvaluationConfig, Relationship, ScratchpadEntry,
    SystemModel, bottlenecks, builtin_catalogue, evaluate,
};

fn component(id: &str, component_type: &str, properties: &[(&str, &str)]) -> Component {
    Component {
        id: ComponentId::new(id),
        name: id.to_owned(),
        component_type: serde_yaml_ng::from_str(component_type).expect("type id"),
        properties: properties
            .iter()
            .map(|(name, source)| ((*name).to_owned(), (*source).to_owned()))
            .collect(),
    }
}

fn link(from: &str, to: &str) -> Relationship {
    Relationship {
        from: ComponentId::new(from),
        to: ComponentId::new(to),
        summary: String::new(),
    }
}

fn config() -> EvaluationConfig {
    EvaluationConfig {
        seed: 7,
        sample_count: 500,
        ..EvaluationConfig::default()
    }
}

fn solve(model: &SystemModel, config: EvaluationConfig) -> BTreeMap<ComponentId, Channels> {
    let catalogue = builtin_catalogue().expect("catalogue");
    let evaluation = evaluate(model, &catalogue, config).expect("solves");
    assert!(evaluation.converged(), "model did not settle");
    evaluation
        .settled()
        .components
        .iter()
        .map(|(id, state)| (id.clone(), Channels(state.channels.clone())))
        .collect()
}

struct Channels(BTreeMap<String, Value>);

impl Channels {
    fn mean(&self, name: &str) -> f64 {
        match self
            .0
            .get(name)
            .unwrap_or_else(|| panic!("no channel '{name}'"))
        {
            Value::Number(value) => *value,
            Value::Distribution(value) => value.mean().expect("mean"),
            value => panic!("channel '{name}' produced {value:?}"),
        }
    }

    fn spread(&self, name: &str) -> f64 {
        match self
            .0
            .get(name)
            .unwrap_or_else(|| panic!("no channel '{name}'"))
        {
            Value::Number(_) => 0.0,
            Value::Distribution(value) => value.stdev().expect("stdev"),
            value => panic!("channel '{name}' produced {value:?}"),
        }
    }
}

/// Demand flows from a source through a pool and the pool's capacity follows
/// Little's Law.
#[test]
fn a_two_component_model_solves_to_its_capacity() {
    let model = SystemModel {
        components: vec![
            component("users", "client", &[("request_rate", "100")]),
            component(
                "api",
                "compute",
                &[
                    ("service_time", "0.02"),
                    ("parallelism", "8"),
                    ("replicas", "4"),
                ],
            ),
        ],
        relationships: vec![link("users", "api")],
        ..SystemModel::default()
    };
    let solved = solve(&model, config());
    let api = &solved[&ComponentId::new("api")];

    // 32 concurrent slots at 20 ms each sustain 1600 requests per second.
    assert!((api.mean("capacity") - 1_600.0).abs() < 1e-6);
    assert!((api.mean("offered") - 100.0).abs() < 1e-9);
    assert!((api.mean("utilisation") - 0.0625).abs() < 1e-9);
    // Well below saturation the pool serves everything offered.
    assert!((api.mean("throughput") - 100.0).abs() < 1e-9);
}

/// Little's Law holds on the solved quantities, not merely inside the manifest.
#[test]
fn solved_concurrency_matches_littles_law() {
    let model = SystemModel {
        components: vec![
            component("users", "client", &[("request_rate", "400")]),
            component(
                "api",
                "compute",
                &[("service_time", "0.05"), ("parallelism", "40")],
            ),
        ],
        relationships: vec![link("users", "api")],
        ..SystemModel::default()
    };
    let solved = solve(&model, config());
    let api = &solved[&ComponentId::new("api")];
    let expected = api.mean("throughput") * api.mean("residence");
    assert!(
        (api.mean("concurrency") - expected).abs() < 1e-6,
        "concurrency {} against rate times residence {expected}",
        api.mean("concurrency")
    );
}

/// A saturated pool clamps throughput and the excess shows as lost demand.
#[test]
fn demand_beyond_capacity_is_clamped() {
    let model = SystemModel {
        components: vec![
            component("users", "client", &[("request_rate", "5000")]),
            component(
                "api",
                "compute",
                &[("service_time", "0.02"), ("parallelism", "8")],
            ),
        ],
        relationships: vec![link("users", "api")],
        ..SystemModel::default()
    };
    let solved = solve(&model, config());
    let api = &solved[&ComponentId::new("api")];
    assert!((api.mean("capacity") - 400.0).abs() < 1e-6);
    assert!((api.mean("throughput") - 400.0).abs() < 1e-6);
    assert!(api.mean("utilisation") > 0.99, "the pool must be saturated");
    // Queueing delay explodes as utilisation approaches one.
    assert!(api.mean("wait") > api.mean("residence") * 0.9);
}

/// Uncertainty in a property reaches every quantity derived from it.
#[test]
fn uncertainty_propagates_through_the_model() {
    let model = SystemModel {
        components: vec![
            component("users", "client", &[("request_rate", "lognormal(5, 0.4)")]),
            component(
                "api",
                "compute",
                &[("service_time", "0.02"), ("parallelism", "16")],
            ),
        ],
        relationships: vec![link("users", "api")],
        ..SystemModel::default()
    };
    let solved = solve(&model, config());
    let api = &solved[&ComponentId::new("api")];
    assert!(api.spread("offered") > 0.0, "demand must stay uncertain");
    assert!(
        api.spread("utilisation") > 0.0,
        "uncertainty must reach utilisation"
    );
    assert!(
        api.spread("wait") > 0.0,
        "uncertainty must reach queueing delay"
    );
    // Capacity depends on no uncertain property, so it stays certain.
    assert_eq!(api.spread("capacity"), 0.0);
}

/// A scratchpad quantity is shared by every component that refers to it.
#[test]
fn scratchpad_quantities_are_shared() {
    let model = SystemModel {
        scratchpad: vec![ScratchpadEntry {
            name: "peak_rate".to_owned(),
            expression: "250".to_owned(),
            unit: Some("op/s".to_owned()),
            summary: String::new(),
        }],
        components: vec![
            component("users", "client", &[("request_rate", "peak_rate")]),
            component(
                "api",
                "compute",
                &[("service_time", "0.01"), ("parallelism", "8")],
            ),
        ],
        relationships: vec![link("users", "api")],
    };
    let solved = solve(&model, config());
    assert!((solved[&ComponentId::new("api")].mean("offered") - 250.0).abs() < 1e-9);
}

/// Demand from several sources arrives summed.
#[test]
fn inbound_demand_is_summed_across_relationships() {
    let model = SystemModel {
        components: vec![
            component("web", "client", &[("request_rate", "120")]),
            component("mobile", "client", &[("request_rate", "80")]),
            component(
                "api",
                "compute",
                &[("service_time", "0.01"), ("parallelism", "32")],
            ),
        ],
        relationships: vec![link("web", "api"), link("mobile", "api")],
        ..SystemModel::default()
    };
    let solved = solve(&model, config());
    assert!((solved[&ComponentId::new("api")].mean("offered") - 200.0).abs() < 1e-9);
}

/// A datastore's resident volume follows the ingest rate and retention window.
#[test]
fn retention_sets_resident_volume() {
    let model = SystemModel {
        components: vec![
            component("writers", "client", &[("request_rate", "50")]),
            component(
                "store",
                "datastore",
                &[
                    ("operation_limit", "10000"),
                    ("transfer_limit", "1e9"),
                    ("volume_limit", "1e12"),
                    ("record_size", "2048"),
                    ("retention", "86400"),
                ],
            ),
        ],
        relationships: vec![link("writers", "store")],
        ..SystemModel::default()
    };
    let solved = solve(&model, config());
    let store = &solved[&ComponentId::new("store")];
    // 50 records per second retained for a day, at 2 KiB each.
    assert!((store.mean("records") - 50.0 * 86_400.0).abs() < 1e-3);
    assert!((store.mean("volume") - 50.0 * 86_400.0 * 2048.0).abs() < 1.0);
    assert!((store.mean("transfer") - 50.0 * 2048.0).abs() < 1e-6);
}

/// A model wired into a cycle still settles, because the solver relaxes.
#[test]
fn a_feedback_loop_settles() {
    let model = SystemModel {
        components: vec![
            component("users", "client", &[("request_rate", "40")]),
            component(
                "api",
                "compute",
                &[("service_time", "0.01"), ("parallelism", "16")],
            ),
            component(
                "cache",
                "compute",
                &[("service_time", "0.002"), ("parallelism", "16")],
            ),
        ],
        relationships: vec![
            link("users", "api"),
            link("api", "cache"),
            link("cache", "api"),
        ],
        ..SystemModel::default()
    };
    let catalogue = builtin_catalogue().expect("catalogue");
    let evaluation = evaluate(&model, &catalogue, config()).expect("solves");
    assert!(
        evaluation.converged(),
        "a contracting loop must settle, moved {}",
        evaluation.settled().movement
    );
}

/// Structural mistakes are reported against the component that made them.
#[test]
fn authoring_mistakes_are_reported() {
    let catalogue = builtin_catalogue().expect("catalogue");

    let missing = SystemModel {
        components: vec![component("api", "compute", &[("service_time", "0.01")])],
        ..SystemModel::default()
    };
    let error = evaluate(&missing, &catalogue, config()).expect_err("missing property");
    assert!(error.to_string().contains("parallelism"), "{error}");

    let unknown = SystemModel {
        components: vec![component(
            "users",
            "client",
            &[("request_rate", "10"), ("rate", "10")],
        )],
        ..SystemModel::default()
    };
    let error = evaluate(&unknown, &catalogue, config()).expect_err("unknown property");
    assert!(error.to_string().contains("does not declare"), "{error}");

    let absent = SystemModel {
        components: vec![component("api", "nonexistent", &[])],
        ..SystemModel::default()
    };
    let error = evaluate(&absent, &catalogue, config()).expect_err("unknown type");
    assert!(error.to_string().contains("unknown type"), "{error}");
}

fn ranked(model: &SystemModel) -> Vec<Bottleneck> {
    let catalogue = builtin_catalogue().expect("catalogue");
    let evaluation = evaluate(model, &catalogue, config()).expect("solves");
    bottlenecks(model, &catalogue, evaluation.settled(), config()).expect("ranks")
}

/// The binding constraint is identified, not merely the busiest component.
///
/// The store has ample headroom on every one of its three limits while the pool
/// is far past its own, so the report must name the pool's capacity rather than
/// whichever component happens to carry the largest numbers.
#[test]
fn the_binding_constraint_is_ranked_first() {
    let model = SystemModel {
        components: vec![
            component("users", "client", &[("request_rate", "900")]),
            component(
                "api",
                "compute",
                &[("service_time", "0.02"), ("parallelism", "8")],
            ),
            component(
                "store",
                "datastore",
                &[
                    ("operation_limit", "100000"),
                    ("transfer_limit", "1e10"),
                    ("volume_limit", "1e14"),
                    ("record_size", "512"),
                    ("retention", "3600"),
                ],
            ),
        ],
        relationships: vec![link("users", "api"), link("api", "store")],
        ..SystemModel::default()
    };
    let ranked = ranked(&model);
    let worst = &ranked[0];
    assert_eq!(worst.component.as_str(), "api");
    assert_eq!(worst.constraint, "capacity");
    assert!(worst.binds(), "the pool must be reported as binding");
    assert!(worst.utilisation > 2.0, "900 offered against 400 served");
    assert!(worst.headroom < 0.0, "a bound constraint has no headroom");
    assert!(
        !worst.summary.trim().is_empty(),
        "a report must say why the limit matters"
    );

    // The store is present but unstressed, so it ranks below.
    let store = ranked
        .iter()
        .find(|entry| entry.component.as_str() == "store" && entry.constraint == "operations")
        .expect("store constraint");
    assert!(!store.binds());
    assert!(store.utilisation < 0.05);
}

/// Which limit binds first depends on record size, not on the store.
///
/// The same store bottlenecks on operation rate for many small records and on
/// transfer rate for few large ones. A model that tracked only one limit could
/// not express that, and an engineer reasoning informally routinely gets it
/// wrong.
#[test]
fn record_size_decides_which_storage_limit_binds() {
    let store = |record_size: &str, rate: &str| SystemModel {
        components: vec![
            component("writers", "client", &[("request_rate", rate)]),
            component(
                "store",
                "datastore",
                &[
                    ("operation_limit", "20000"),
                    ("transfer_limit", "2e8"),
                    ("volume_limit", "1e15"),
                    ("record_size", record_size),
                    ("retention", "60"),
                ],
            ),
        ],
        relationships: vec![link("writers", "store")],
        ..SystemModel::default()
    };

    let small = ranked(&store("64", "19000"));
    assert_eq!(small[0].constraint, "operations", "{small:#?}");

    let large = ranked(&store("65536", "4000"));
    assert_eq!(large[0].constraint, "transfer", "{large:#?}");
}

/// Uncertainty can make a constraint bind while its average says otherwise.
///
/// This is the failure a mean-only model cannot see: average utilisation sits
/// comfortably below one, yet a substantial share of draws has already crossed
/// the limit. Reporting the probability of binding alongside the mean is what
/// makes that visible.
#[test]
fn a_constraint_can_bind_while_its_mean_looks_healthy() {
    let model = SystemModel {
        components: vec![
            component(
                "users",
                "client",
                &[("request_rate", "lognormal(5.4, 0.6)")],
            ),
            component(
                "api",
                "compute",
                &[("service_time", "0.01"), ("parallelism", "4")],
            ),
        ],
        relationships: vec![link("users", "api")],
        ..SystemModel::default()
    };
    let ranked = ranked(&model);
    let capacity = ranked
        .iter()
        .find(|entry| entry.constraint == "capacity")
        .expect("capacity constraint");
    assert!(
        capacity.utilisation < 1.0,
        "average utilisation should look survivable, got {}",
        capacity.utilisation
    );
    assert!(
        capacity.probability_of_binding > 0.05,
        "a real share of draws must already saturate, got {}",
        capacity.probability_of_binding
    );
    assert!(
        capacity.utilisation_p90 > capacity.utilisation,
        "the upper tail must sit above the mean"
    );
}

/// Every constraint in the model is reported, whether or not it binds.
#[test]
fn every_constraint_is_accounted_for() {
    let model = SystemModel {
        components: vec![
            component("users", "client", &[("request_rate", "10")]),
            component(
                "gateway",
                "load-balancer",
                &[
                    ("replicas", "3"),
                    ("admission_limit", "1000"),
                    ("connection_limit", "500"),
                    ("overhead", "0.001"),
                ],
            ),
        ],
        relationships: vec![link("users", "gateway")],
        ..SystemModel::default()
    };
    let ranked = ranked(&model);
    let names = ranked
        .iter()
        .map(|entry| entry.constraint.as_str())
        .collect::<Vec<_>>();
    assert!(names.contains(&"admission"), "{names:?}");
    assert!(names.contains(&"connections"), "{names:?}");
    assert!(ranked.iter().all(|entry| !entry.binds()));
}
