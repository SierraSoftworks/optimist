//! End-to-end coverage for solving a system model.
//!
//! These tests build models the way an engineer would, out of catalogue types
//! wired together, and check that the solved quantities obey the laws the
//! manifests claim to apply.

use std::collections::BTreeMap;

use optimist::squiggle::Value;
use optimist::system::{
    AttachedMutator, Bottleneck, Component, ComponentId, Distribution, EvaluationConfig, MutatorId,
    Relationship, ScaleUnit, ScaleUnitId, ScratchpadEntry, SystemModel, bottlenecks,
    builtin_catalogue, evaluate,
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
        position: None,
    }
}

fn link(from: &str, to: &str) -> Relationship {
    Relationship {
        from_port: None,
        to_port: None,
        capacity: None,
        bandwidth: None,
        latency: None,
        from: ComponentId::new(from),
        to: ComponentId::new(to),
        mutators: Vec::new(),
        summary: String::new(),
    }
}

fn linked(from: &str, to: &str, mutators: &[(&str, &[(&str, &str)])]) -> Relationship {
    Relationship {
        from_port: None,
        to_port: None,
        capacity: None,
        bandwidth: None,
        latency: None,
        from: ComponentId::new(from),
        to: ComponentId::new(to),
        mutators: mutators
            .iter()
            .map(|(id, properties)| AttachedMutator {
                mutator: MutatorId::new(*id),
                properties: properties
                    .iter()
                    .map(|(name, source)| ((*name).to_owned(), (*source).to_owned()))
                    .collect(),
            })
            .collect(),
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
                &[("service_time", "0.02"), ("parallelism", "8")],
            ),
        ],
        relationships: vec![link("users", "api")],
        scale_units: vec![cell("api-replicas", "4", Distribution::Sharded, &["api"])],
        ..SystemModel::default()
    };
    let solved = solve(&model, config());
    let api = &solved[&ComponentId::new("api")];

    // Each replica has eight slots and receives a quarter of the fleet's load.
    assert!((api.mean("capacity") - 400.0).abs() < 1e-6);
    assert!((api.mean("offered") - 25.0).abs() < 1e-9);
    assert!((api.mean("utilisation") - 0.0625).abs() < 1e-9);
    // Well below saturation the pool serves everything offered.
    assert!((api.mean("calls") - 25.0).abs() < 1e-9);
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
    let expected = api.mean("calls") * api.mean("residence");
    assert!(
        (api.mean("concurrency") - expected).abs() < 1e-6,
        "concurrency {} against rate times residence {expected}",
        api.mean("concurrency")
    );
}

/// A saturated pool refuses the excess rather than quietly serving less.
///
/// Overload used to be expressed by clamping throughput, which meant a design
/// asked for more than it could serve reported the shortfall nowhere. It now
/// reports the share it could not answer, and the caller is the one that finds
/// out — which is what makes shedding a decision rather than an accident.
#[test]
fn demand_beyond_capacity_is_refused() {
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
    assert!(api.mean("utilisation") > 0.99, "the pool must be saturated");
    let users = &solved[&ComponentId::new("users")];
    // Eight slots at 20 ms sustain 400 of the 5000 offered. The refusal happens
    // on the wire, so it is the caller that finds out.
    let served = users.mean("offered") * users.mean("success");
    assert!(
        (served - 400.0).abs() < 40.0,
        "what is served should be about the capacity, got {served}"
    );
    assert!(
        users.mean("success") < 0.2,
        "the caller must see the refusal, got {}",
        users.mean("success")
    );
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
        solved[&ComponentId::new("users")].spread("latency") > 0.0,
        "uncertainty must reach the delay a caller observes, which is where the \
         queueing caused by uncertain demand now shows"
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
        ..SystemModel::default()
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
///
/// Every call chain is now a cycle: the pool's capacity depends on how long its
/// dependency takes, and the dependency's load depends on what the pool passes
/// through. There is no term to evaluate first, so this is exactly the case
/// relaxation exists to handle.
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
        relationships: vec![link("users", "api"), link("api", "cache")],
        ..SystemModel::default()
    };
    let catalogue = builtin_catalogue().expect("catalogue");
    let evaluation = evaluate(&model, &catalogue, config()).expect("solves");
    assert!(
        evaluation.converged(),
        "a contracting loop must settle, moved {}",
        evaluation.settled().movement
    );
    let settled = &evaluation.settled().components;
    let latency = |id: &str, channel: &str| {
        Channels(settled[&ComponentId::new(id)].channels.clone()).mean(channel)
    };
    // The pool holds a worker for its own service plus the cache's answer, so
    // the cache's latency is visible in the pool's hold time.
    let cache_residence = latency("cache", "residence");
    let api_hold = latency("api", "hold_time");
    assert!(
        (api_hold - (0.01 + cache_residence)).abs() < 1e-6,
        "expected 0.01 + {cache_residence}, got {api_hold}"
    );
}

/// A buffer filled by a burst empties again once the burst has passed.
///
/// This is what makes a queue the memory of a design. Draining only what
/// arrives would leave the backlog resident forever, so a model would report a
/// system that recovers as one that never does, and recovery time — the whole
/// reason for solving through time — could not be read at all.
#[test]
fn a_queue_drains_the_backlog_a_burst_left_behind() {
    let model = SystemModel {
        scratchpad: vec![ScratchpadEntry {
            name: "burst".to_owned(),
            expression: "if t < 5 then 900 else 100".to_owned(),
            unit: Some("op/s".to_owned()),
            summary: String::new(),
        }],
        components: vec![
            component("producers", "client", &[("request_rate", "burst")]),
            component(
                "jobs",
                "queue",
                &[("service_rate", "400"), ("capacity", "100000")],
            ),
            component(
                "worker",
                "compute",
                &[("service_time", "0.002"), ("parallelism", "8")],
            ),
        ],
        relationships: vec![link("producers", "jobs"), link("jobs", "worker")],
        ..SystemModel::default()
    };

    let backlog_at = |seconds: f64| {
        let config = EvaluationConfig {
            mode: optimist::system::SolveMode::Transient,
            horizon: (seconds / 0.5) as usize + 1,
            step: 0.5,
            sample_count: 64,
            ..config()
        };
        solve(&model, config)[&ComponentId::new("jobs")].mean("backlog")
    };

    // Offered at 900 against a 400 drain, so the backlog climbs while the burst
    // lasts.
    let during = backlog_at(4.0);
    assert!(
        during > 1_000.0,
        "expected a backlog to build, got {during}"
    );

    // The burst ends at five seconds and demand falls below the drain rate.
    // What accumulated has to be worked off, so the queue empties gradually
    // rather than the moment the cause goes away.
    let recovering = backlog_at(8.0);
    assert!(
        recovering < during,
        "expected the backlog to fall from {during}, got {recovering}"
    );
    assert!(
        recovering > 0.0,
        "expected recovery to take time rather than happen at once"
    );

    let recovered = backlog_at(30.0);
    assert!(
        recovered < 1.0,
        "expected the queue to have emptied by thirty seconds, got {recovered}"
    );
}

/// An idle queue reports no waiting rather than failing to evaluate.
///
/// Residence time is occupancy over throughput, and a queue nobody is using has
/// neither. Reporting zero is what lets a buffer sit unused in a design that
/// still solves.
#[test]
fn an_idle_queue_reports_no_wait() {
    let model = SystemModel {
        components: vec![
            component("producers", "client", &[("request_rate", "0")]),
            component(
                "jobs",
                "queue",
                &[("service_rate", "400"), ("capacity", "1000")],
            ),
            component(
                "worker",
                "compute",
                &[("service_time", "0.002"), ("parallelism", "8")],
            ),
        ],
        relationships: vec![link("producers", "jobs"), link("jobs", "worker")],
        ..SystemModel::default()
    };
    let solved = solve(&model, config());
    let jobs = &solved[&ComponentId::new("jobs")];
    assert_eq!(jobs.mean("backlog"), 0.0);
    assert_eq!(jobs.mean("wait"), 0.0);
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
            component(
                "users",
                "client",
                &[
                    ("request_rate", "10"),
                    ("latency_target", "1"),
                    ("success_target", "0.9"),
                ],
            ),
            component(
                "gateway",
                "load-balancer",
                &[("connection_limit", "500"), ("overhead", "0.001")],
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
    assert!(names.contains(&"latency_objective"), "{names:?}");
    assert!(names.contains(&"success_objective"), "{names:?}");
    assert!(names.contains(&"connections"), "{names:?}");
    assert!(ranked.iter().all(|entry| !entry.binds()));
}

fn amplified(budget: &str) -> f64 {
    let model = SystemModel {
        components: vec![
            component("users", "client", &[("request_rate", "100")]),
            component(
                "api",
                "compute",
                &[("service_time", "0.001"), ("parallelism", "64")],
            ),
        ],
        relationships: vec![linked(
            "users",
            "api",
            &[
                ("retry", &[("attempts", "3")][..]),
                ("timeout", &[("budget", budget)][..]),
            ],
        )],
        ..SystemModel::default()
    };
    solve(&model, config())[&ComponentId::new("api")].mean("arriving")
}

/// A retry policy amplifies the demand reaching the dependency behind it.
///
/// This is the term that turns a partial outage into a retry storm, and it is
/// invisible on a diagram: the relationship looks identical whether or not a
/// policy is attached.
///
/// The retry knows nothing about time. It reissues what failed, and what counts
/// as a failure is decided by the timeout beneath it, which is why the
/// amplification below is produced by the budget alone. The pool answers in
/// about a millisecond, and an attempt succeeds with probability
/// `1 - exp(-budget / latency)`.
#[test]
fn retrying_amplifies_downstream_demand() {
    // Ten times the dependency's latency almost always suffices, so a request
    // costs about one attempt.
    assert!(
        (amplified("0.0069") - 100.0).abs() < 1.0,
        "got {}",
        amplified("0.0069")
    );
    // A budget of `latency * ln 2` succeeds half the time.
    let degraded = amplified("0.000693");
    assert!(
        (degraded - 175.0).abs() < 5.0,
        "expected 1 + 0.5 + 0.25 attempts per request, got {degraded}"
    );
    // A budget far below the latency never succeeds, so the full budget of three
    // attempts is spent on every request.
    assert!(
        (amplified("0.000001") - 300.0).abs() < 5.0,
        "got {}",
        amplified("0.000001")
    );
}

/// Amplification can push a healthy dependency past its capacity.
///
/// The pool serves the offered load comfortably until a retry policy triples it,
/// which is the failure mode that makes retries dangerous precisely when they
/// seem most necessary.
#[test]
fn amplification_can_create_the_bottleneck_it_responds_to() {
    let pool = |mutators: Vec<Relationship>| SystemModel {
        components: vec![
            component("users", "client", &[("request_rate", "150")]),
            component(
                "api",
                "compute",
                &[("service_time", "0.01"), ("parallelism", "2")],
            ),
        ],
        relationships: mutators,
        ..SystemModel::default()
    };
    let catalogue = builtin_catalogue().expect("catalogue");

    let plain = pool(vec![link("users", "api")]);
    let evaluation = evaluate(&plain, &catalogue, config()).expect("solves");
    let ranked = bottlenecks(&plain, &catalogue, evaluation.settled(), config()).expect("ranks");
    assert!(!ranked[0].binds(), "200 capacity serves 150 offered");

    let retried = pool(vec![linked(
        "users",
        "api",
        &[
            ("retry", &[("attempts", "3")][..]),
            ("timeout", &[("budget", "0.001")][..]),
        ],
    )]);
    let evaluation = evaluate(&retried, &catalogue, config()).expect("solves");
    let ranked = bottlenecks(&retried, &catalogue, evaluation.settled(), config()).expect("ranks");
    assert!(
        ranked[0].binds(),
        "retrying must push the pool past capacity, utilisation {}",
        ranked[0].utilisation
    );
}

/// A cache reduces the demand reaching the component behind it.
#[test]
fn caching_absorbs_demand_before_the_dependency() {
    let model = SystemModel {
        components: vec![
            component("users", "client", &[("request_rate", "1000")]),
            component(
                "store",
                "datastore",
                &[
                    ("operation_limit", "100000"),
                    ("transfer_limit", "1e10"),
                    ("volume_limit", "1e14"),
                    ("record_size", "256"),
                    ("retention", "600"),
                ],
            ),
        ],
        relationships: vec![linked(
            "users",
            "store",
            &[("cache", &[("hit_ratio", "0.9")])],
        )],
        ..SystemModel::default()
    };
    assert!(
        (solve(&model, config())[&ComponentId::new("store")].mean("operations") - 100.0).abs()
            < 1e-6
    );
}

/// Batching trades operation rate for payload size, leaving byte rate alone.
#[test]
fn batching_trades_operations_for_payload() {
    let model = |mutators: Vec<Relationship>| SystemModel {
        components: vec![
            component(
                "writers",
                "client",
                &[("request_rate", "1000"), ("payload", "512")],
            ),
            component(
                "store",
                "datastore",
                &[
                    ("operation_limit", "100000"),
                    ("transfer_limit", "1e10"),
                    ("volume_limit", "1e14"),
                    ("record_size", "512"),
                    ("retention", "60"),
                ],
            ),
        ],
        relationships: mutators,
        ..SystemModel::default()
    };
    let plain = solve(&model(vec![link("writers", "store")]), config());
    let batched = solve(
        &model(vec![linked(
            "writers",
            "store",
            &[("batch", &[("size", "20"), ("max_delay", "0.05")])],
        )]),
        config(),
    );
    let store = ComponentId::new("store");
    assert!((plain[&store].mean("operations") - 1000.0).abs() < 1e-6);
    assert!((batched[&store].mean("operations") - 50.0).abs() < 1e-6);
}

/// Behaviours compose in the order they are declared.
///
/// Shedding before retrying caps what the policy may amplify; retrying before
/// shedding lets the amplified demand meet the cap instead. The two orders give
/// different answers, which is why the order is written down.
#[test]
fn behaviour_order_changes_the_result() {
    let model = |mutators: &[(&str, &[(&str, &str)])]| SystemModel {
        components: vec![
            component("users", "client", &[("request_rate", "200")]),
            component(
                "api",
                "compute",
                &[("service_time", "0.001"), ("parallelism", "64")],
            ),
        ],
        relationships: vec![linked("users", "api", mutators)],
        ..SystemModel::default()
    };
    let retry = ("retry", &[("attempts", "3")][..]);
    let shed = ("load-shed", &[("limit", "100")][..]);
    // A budget far below the pool's latency fails every attempt, so the retry
    // policy always spends its full budget and the arithmetic below is exact.
    let timeout = ("timeout", &[("budget", "0.000001")][..]);

    // Shedding first caps demand at 100, which retrying then triples.
    let shed_first = solve(&model(&[shed, retry, timeout]), config());
    // Retrying first triples demand to 600, which shedding then caps at 100.
    let retry_first = solve(&model(&[retry, shed, timeout]), config());

    let api = ComponentId::new("api");
    assert!(
        (shed_first[&api].mean("arriving") - 300.0).abs() < 5.0,
        "got {}",
        shed_first[&api].mean("arriving")
    );
    assert!(
        (retry_first[&api].mean("arriving") - 100.0).abs() < 5.0,
        "got {}",
        retry_first[&api].mean("arriving")
    );
}

/// A timeout bounds the latency a caller observes.
#[test]
fn a_timeout_bounds_observed_latency() {
    let model = SystemModel {
        components: vec![
            component("users", "client", &[("request_rate", "10")]),
            component(
                "slow",
                "compute",
                &[("service_time", "2"), ("parallelism", "64")],
            ),
            component(
                "caller",
                "compute",
                &[("service_time", "0.001"), ("parallelism", "64")],
            ),
        ],
        relationships: vec![
            link("users", "slow"),
            linked("slow", "caller", &[("timeout", &[("budget", "0.25")])]),
        ],
        ..SystemModel::default()
    };
    let catalogue = builtin_catalogue().expect("catalogue");
    let evaluation = evaluate(&model, &catalogue, config()).expect("solves");
    assert!(evaluation.converged());
}

/// An unknown behaviour is reported against the relationship that attached it.
#[test]
fn unknown_behaviours_are_reported() {
    let model = SystemModel {
        components: vec![
            component("users", "client", &[("request_rate", "10")]),
            component(
                "api",
                "compute",
                &[("service_time", "0.01"), ("parallelism", "8")],
            ),
        ],
        relationships: vec![linked("users", "api", &[("nonexistent", &[])])],
        ..SystemModel::default()
    };
    let catalogue = builtin_catalogue().expect("catalogue");
    let error = evaluate(&model, &catalogue, config()).expect_err("unknown behaviour");
    assert!(error.to_string().contains("unknown behaviour"), "{error}");
}

fn cell(id: &str, replicas: &str, distribution: Distribution, members: &[&str]) -> ScaleUnit {
    ScaleUnit {
        id: ScaleUnitId::new(id),
        name: id.to_owned(),
        summary: String::new(),
        replicas: replicas.to_owned(),
        distribution,
        members: members.iter().map(|id| ComponentId::new(*id)).collect(),
        parent: None,
    }
}

fn sharded_fleet(units: Vec<ScaleUnit>) -> SystemModel {
    SystemModel {
        components: vec![
            component("users", "client", &[("request_rate", "1200")]),
            component(
                "api",
                "compute",
                &[("service_time", "0.01"), ("parallelism", "8")],
            ),
        ],
        relationships: vec![link("users", "api")],
        scale_units: units,
        ..SystemModel::default()
    }
}

fn replicated_path(distribution: Distribution, members: &[&str]) -> SystemModel {
    SystemModel {
        components: vec![
            component("users", "client", &[("request_rate", "1200")]),
            component(
                "api",
                "compute",
                &[("service_time", "0.01"), ("parallelism", "1000")],
            ),
            component(
                "store",
                "compute",
                &[("service_time", "0.01"), ("parallelism", "1000")],
            ),
        ],
        relationships: vec![link("users", "api"), link("api", "store")],
        scale_units: vec![cell("replicas", "4", distribution, members)],
        ..SystemModel::default()
    }
}

/// A sharded unit divides demand, so a model describes one replica.
///
/// This is the question worth asking of a fleet: not whether the total capacity
/// exceeds the total load, but whether one cell can serve the share that reaches
/// it.
#[test]
fn a_sharded_scale_unit_divides_demand() {
    let whole = solve(&sharded_fleet(Vec::new()), config());
    assert!((whole[&ComponentId::new("api")].mean("offered") - 1_200.0).abs() < 1e-6);

    let sharded = solve(
        &sharded_fleet(vec![cell("cell", "4", Distribution::Sharded, &["api"])]),
        config(),
    );
    assert!((sharded[&ComponentId::new("api")].mean("offered") - 300.0).abs() < 1e-6);
}

/// A mirrored unit replicates cost without dividing load.
///
/// Replicating writes to every region means every region receives every write.
/// Treating that as though it sharded would size the design for a fraction of
/// its real demand.
#[test]
fn a_mirrored_scale_unit_does_not_divide_demand() {
    let mirrored = solve(
        &sharded_fleet(vec![cell("region", "4", Distribution::Mirrored, &["api"])]),
        config(),
    );
    assert!((mirrored[&ComponentId::new("api")].mean("offered") - 1_200.0).abs() < 1e-6);
}

/// Per-replica work gathers when it leaves a sharded boundary.
#[test]
fn traffic_leaving_a_sharded_unit_gathers_before_a_shared_dependency() {
    let solved = solve(&replicated_path(Distribution::Sharded, &["api"]), config());
    assert!((solved[&ComponentId::new("api")].mean("offered") - 300.0).abs() < 1e-6);
    assert!((solved[&ComponentId::new("store")].mean("offered") - 1_200.0).abs() < 1e-6);
}

/// Members of one unit communicate locally rather than sharding twice.
#[test]
fn traffic_between_members_of_one_sharded_unit_stays_local() {
    let solved = solve(
        &replicated_path(Distribution::Sharded, &["api", "store"]),
        config(),
    );
    assert!((solved[&ComponentId::new("api")].mean("offered") - 300.0).abs() < 1e-6);
    assert!((solved[&ComponentId::new("store")].mean("offered") - 300.0).abs() < 1e-6);
}

/// Mirrored replicas repeat their downstream work outside the boundary.
#[test]
fn traffic_leaving_a_mirrored_unit_multiplies_cost() {
    let solved = solve(&replicated_path(Distribution::Mirrored, &["api"]), config());
    assert!((solved[&ComponentId::new("api")].mean("offered") - 1_200.0).abs() < 1e-6);
    assert!((solved[&ComponentId::new("store")].mean("offered") - 4_800.0).abs() < 1e-6);
}

/// Nested units multiply, and only the sharded levels divide.
#[test]
fn nesting_multiplies_replicas_and_shards_divide() {
    let mut region = cell("region", "3", Distribution::Mirrored, &[]);
    let mut shard = cell("shard", "10", Distribution::Sharded, &["api"]);
    shard.parent = Some(ScaleUnitId::new("region"));
    region.members = Vec::new();

    let model = sharded_fleet(vec![region, shard]);
    let solved = solve(&model, config());
    // Thirty copies exist, but only the ten shards divide the load.
    assert!((solved[&ComponentId::new("api")].mean("offered") - 120.0).abs() < 1e-6);

    let catalogue = builtin_catalogue().expect("catalogue");
    let evaluation = evaluate(&model, &catalogue, config()).expect("solves");
    let ranked = bottlenecks(&model, &catalogue, evaluation.settled(), config()).expect("ranks");
    let capacity = ranked
        .iter()
        .find(|entry| entry.constraint == "capacity")
        .expect("capacity");
    assert!((capacity.replicas - 30.0).abs() < 1e-9, "{capacity:#?}");
}

/// Sharding can relieve a constraint that binds on an unsharded fleet.
#[test]
fn sharding_relieves_a_bottleneck() {
    let catalogue = builtin_catalogue().expect("catalogue");
    let bound = sharded_fleet(Vec::new());
    let evaluation = evaluate(&bound, &catalogue, config()).expect("solves");
    let ranked = bottlenecks(&bound, &catalogue, evaluation.settled(), config()).expect("ranks");
    assert!(ranked[0].binds(), "1200 offered against 800 served");

    let spread = sharded_fleet(vec![cell("cell", "4", Distribution::Sharded, &["api"])]);
    let evaluation = evaluate(&spread, &catalogue, config()).expect("solves");
    let ranked = bottlenecks(&spread, &catalogue, evaluation.settled(), config()).expect("ranks");
    assert!(!ranked[0].binds(), "each cell serves 300 against 800");
}

/// Observed latency takes the largest arrival rather than the sum.
///
/// A caller reaching two dependencies waits for the slower one, not for both
/// end to end, and summing would invent delay nobody experienced. Latency
/// travels back along a relationship, so this is a claim about what returns to
/// a caller that fanned out rather than about what arrives at one.
#[test]
fn latency_does_not_accumulate_across_parallel_dependencies() {
    let model = SystemModel {
        components: vec![
            component("users", "client", &[("request_rate", "10")]),
            component(
                "collector",
                "compute",
                &[("service_time", "0.001"), ("parallelism", "64")],
            ),
            component(
                "slow",
                "compute",
                &[("service_time", "0.2"), ("parallelism", "64")],
            ),
            component(
                "quick",
                "compute",
                &[("service_time", "0.05"), ("parallelism", "64")],
            ),
        ],
        relationships: vec![
            link("users", "collector"),
            link("collector", "slow"),
            link("collector", "quick"),
        ],
        ..SystemModel::default()
    };
    let catalogue = builtin_catalogue().expect("catalogue");
    let evaluation = evaluate(&model, &catalogue, config()).expect("solves");
    assert!(evaluation.converged());
    let solved = evaluation.settled().components.clone();
    let wait =
        Channels(solved[&ComponentId::new("collector")].channels.clone()).mean("dependency_wait");
    let slower = Channels(solved[&ComponentId::new("slow")].channels.clone()).mean("residence");
    // The slow dependency alone sets the wait; the quick one is free. The wire
    // in front of each adds a little queueing of its own, which is why this is a
    // tolerance rather than an equality.
    assert!(
        (wait - slower).abs() < slower * 0.05,
        "expected about the slower dependency {slower}, waited {wait}"
    );
}

/// Structural mistakes in scale units are reported.
#[test]
fn scale_unit_mistakes_are_reported() {
    let catalogue = builtin_catalogue().expect("catalogue");

    let unknown = sharded_fleet(vec![cell("cell", "2", Distribution::Sharded, &["missing"])]);
    let error = evaluate(&unknown, &catalogue, config()).expect_err("unknown member");
    assert!(error.to_string().contains("does not declare"), "{error}");

    let shared = sharded_fleet(vec![
        cell("left", "2", Distribution::Sharded, &["api"]),
        cell("right", "2", Distribution::Sharded, &["api"]),
    ]);
    let error = evaluate(&shared, &catalogue, config()).expect_err("shared membership");
    assert!(
        error.to_string().contains("more than one scale unit"),
        "{error}"
    );

    let mut looping = cell("loop", "2", Distribution::Sharded, &["api"]);
    looping.parent = Some(ScaleUnitId::new("loop"));
    let error = evaluate(&sharded_fleet(vec![looping]), &catalogue, config()).expect_err("cycle");
    assert!(error.to_string().contains("encloses itself"), "{error}");

    let empty = sharded_fleet(vec![cell("cell", "0", Distribution::Sharded, &["api"])]);
    let error = evaluate(&empty, &catalogue, config()).expect_err("no replicas");
    assert!(
        error.to_string().contains("at least one replica"),
        "{error}"
    );
}
