//! Coverage for reading and writing a design on disk.
//!
//! The schema is checked by round trip rather than against fixed text: a design
//! written and read back must describe the same system, and the exact bytes in
//! between are an implementation detail nobody should be asserting against.

use std::{fs, path::PathBuf};

use optimist::system::{
    AttachedMutator, Component, ComponentId, Distribution, EvaluationConfig, Intervention,
    InterventionId, MutatorId, Override, Relationship, SCHEMA_VERSION, ScaleUnit, ScaleUnitId,
    ScratchpadEntry, SystemModel, evaluate, read_system, write_system,
};

fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("optimist-schema-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("scratch directory");
    path
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
    }
}

/// A design exercising every part of the schema at once.
fn full_model() -> SystemModel {
    SystemModel {
        scratchpad: vec![
            ScratchpadEntry {
                name: "peak_rate".to_owned(),
                expression: "lognormal(6, 0.3)".to_owned(),
                unit: Some("op/s".to_owned()),
                summary: "Requests at the daily peak.".to_owned(),
            },
            ScratchpadEntry {
                name: "cache_hits".to_owned(),
                expression: "0.8".to_owned(),
                unit: None,
                summary: String::new(),
            },
        ],
        components: vec![
            component("users", "client", &[("request_rate", "peak_rate")]),
            component(
                "api",
                "compute",
                &[("service_time", "0.01"), ("parallelism", "16")],
            ),
            component(
                "store",
                "datastore",
                &[
                    ("operation_limit", "20000"),
                    ("transfer_limit", "1e9"),
                    ("volume_limit", "1e13"),
                    ("record_size", "1024"),
                    ("retention", "3600"),
                ],
            ),
        ],
        relationships: vec![
            Relationship {
                from: ComponentId::new("users"),
                to: ComponentId::new("api"),
                mutators: vec![AttachedMutator {
                    mutator: MutatorId::new("retry"),
                    properties: [
                        ("attempts".to_owned(), "3".to_owned()),
                        ("attempt_success".to_owned(), "0.99".to_owned()),
                    ]
                    .into_iter()
                    .collect(),
                }],
                summary: "Callers reaching the entry point.".to_owned(),
            },
            Relationship {
                from: ComponentId::new("api"),
                to: ComponentId::new("store"),
                mutators: vec![AttachedMutator {
                    mutator: MutatorId::new("cache"),
                    properties: [("hit_ratio".to_owned(), "cache_hits".to_owned())]
                        .into_iter()
                        .collect(),
                }],
                summary: String::new(),
            },
        ],
        scale_units: vec![ScaleUnit {
            id: ScaleUnitId::new("cell"),
            name: "Cell".to_owned(),
            summary: "One deployable unit.".to_owned(),
            replicas: "4".to_owned(),
            distribution: Distribution::Sharded,
            members: vec![ComponentId::new("api")],
            parent: None,
        }],
        interventions: vec![Intervention {
            id: InterventionId::new("warmer_cache"),
            name: "Warm the cache".to_owned(),
            summary: "Raise the hit ratio.".to_owned(),
            overrides: vec![Override {
                name: "cache_hits".to_owned(),
                expression: "0.95".to_owned(),
            }],
        }],
    }
}

fn canonical(model: &SystemModel) -> String {
    serde_yaml_ng::to_string(model).expect("renders")
}

/// A design written and read back describes the same system.
///
/// Persistence returns a canonical ordering rather than the order a model
/// happened to be assembled in, so the comparison is against that.
#[test]
fn a_design_survives_a_round_trip() {
    let directory = scratch("round-trip");
    let model = full_model();
    write_system(&directory, "Checkout", "The checkout path.", &model).expect("writes");

    let loaded = read_system(&directory).expect("reads");
    assert_eq!(loaded.name, "Checkout");
    assert_eq!(loaded.summary, "The checkout path.");
    assert_eq!(canonical(&loaded.model), canonical(&model.canonicalise()));
}

/// Reading a design twice yields exactly the same thing.
#[test]
fn reading_is_idempotent() {
    let directory = scratch("idempotent");
    write_system(&directory, "Checkout", "", &full_model()).expect("writes");
    let once = read_system(&directory).expect("reads");
    write_system(&directory, "Checkout", "", &once.model).expect("writes");
    let twice = read_system(&directory).expect("reads");
    assert_eq!(canonical(&once.model), canonical(&twice.model));
}

/// A round-tripped design still solves to the same answer.
#[test]
fn a_round_tripped_design_solves_identically() {
    let directory = scratch("solves");
    let model = full_model();
    write_system(&directory, "Checkout", "", &model).expect("writes");
    let loaded = read_system(&directory).expect("reads");

    let config = EvaluationConfig {
        seed: 5,
        sample_count: 300,
        ..EvaluationConfig::default()
    };
    let before = evaluate(&model.canonicalise(), &loaded.component_types, config).expect("solves");
    let after = evaluate(&loaded.model, &loaded.component_types, config).expect("solves");

    let api = ComponentId::new("api");
    let offered = |evaluation: &optimist::system::Evaluation| {
        format!(
            "{:?}",
            evaluation.settled().components[&api].channels["offered"]
        )
    };
    assert_eq!(offered(&before), offered(&after));
}

/// Relationships are stored with the component they leave.
#[test]
fn a_relationship_lives_with_its_source() {
    let directory = scratch("layout");
    write_system(&directory, "Checkout", "", &full_model()).expect("writes");

    let users = fs::read_to_string(directory.join("components/users.yaml")).expect("reads");
    assert!(users.contains("to: api"), "{users}");
    // The receiving component says nothing about connections arriving at it.
    let api = fs::read_to_string(directory.join("components/api.yaml")).expect("reads");
    assert!(!api.contains("users"), "{api}");
    assert!(api.contains("to: store"), "{api}");
}

/// Writing twice removes components the design no longer contains.
///
/// A stale document would be read back as a component nobody declared, which is
/// worse than a missing one because it would solve without complaint.
#[test]
fn removed_components_do_not_linger() {
    let directory = scratch("prune");
    write_system(&directory, "Checkout", "", &full_model()).expect("writes");
    assert!(directory.join("components/store.yaml").exists());

    let mut trimmed = full_model();
    trimmed
        .components
        .retain(|component| component.id.as_str() != "store");
    trimmed
        .relationships
        .retain(|relationship| relationship.to.as_str() != "store");
    write_system(&directory, "Checkout", "", &trimmed).expect("writes");

    assert!(!directory.join("components/store.yaml").exists());
    assert_eq!(
        read_system(&directory)
            .expect("reads")
            .model
            .components
            .len(),
        2
    );
}

/// A project may define component types the catalogue never anticipated.
#[test]
fn project_local_definitions_are_loaded() {
    let directory = scratch("local-types");
    let mut model = full_model();
    model
        .components
        .push(component("meter", "rate-limiter", &[("ceiling", "500")]));
    model.relationships.push(Relationship {
        from: ComponentId::new("api"),
        to: ComponentId::new("meter"),
        mutators: Vec::new(),
        summary: String::new(),
    });
    write_system(&directory, "Checkout", "", &model).expect("writes");

    fs::create_dir_all(directory.join("component-types")).expect("directory");
    fs::write(
        directory.join("component-types/rate-limiter.yaml"),
        "id: rate-limiter\n\
         name: Rate limiter\n\
         summary: Caps the rate reaching whatever follows it.\n\
         properties:\n\
         \x20 ceiling:\n\
         \x20   unit: op/s\n\
         \x20   summary: Rate above which calls are refused.\n\
         channels:\n\
         \x20 admitted:\n\
         \x20   unit: op/s\n\
         \x20   summary: Calls allowed through.\n\
         \x20   expression: min([inbound.rate, ceiling])\n\
         outputs:\n\
         \x20 rate: admitted\n\
         constraints:\n\
         \x20 ceiling:\n\
         \x20   summary: Demand against the cap.\n\
         \x20   demand: inbound.rate\n\
         \x20   limit: ceiling\n",
    )
    .expect("writes");

    let loaded = read_system(&directory).expect("reads");
    assert!(loaded.component_types.contains_key("rate-limiter"));
    // The design solves against a type the shipped catalogue never knew about.
    evaluate(
        &loaded.model,
        &loaded.component_types,
        EvaluationConfig {
            sample_count: 200,
            ..EvaluationConfig::default()
        },
    )
    .expect("solves");
}

/// A project-local definition that does not validate is rejected on load.
#[test]
fn an_invalid_local_definition_is_rejected() {
    let directory = scratch("bad-type");
    write_system(&directory, "Checkout", "", &full_model()).expect("writes");
    fs::create_dir_all(directory.join("component-types")).expect("directory");
    fs::write(
        directory.join("component-types/broken.yaml"),
        "id: broken\nname: Broken\nchannels:\n  served:\n    unit: op/s\n    expression: nonexistent\n",
    )
    .expect("writes");

    let error = read_system(&directory).expect_err("invalid definition");
    assert!(error.to_string().contains("nonexistent"), "{error}");
}

/// A misspelt field is refused rather than silently defaulted.
///
/// A document that nearly parses is the dangerous case: dropping a misspelt
/// property would leave the model using a default while its author believed
/// otherwise, and every number downstream would look plausible.
#[test]
fn unknown_fields_are_refused() {
    let directory = scratch("strict");
    write_system(&directory, "Checkout", "", &full_model()).expect("writes");
    let path = directory.join("components/api.yaml");
    let document = fs::read_to_string(&path).expect("reads");
    fs::write(&path, format!("{document}parallelisms: '32'\n")).expect("writes");

    let error = read_system(&directory).expect_err("unknown field");
    assert!(error.to_string().contains("parallelisms"), "{error}");
}

/// A directory from the previous schema is refused rather than converted.
#[test]
fn an_older_schema_is_refused() {
    let directory = scratch("old-schema");
    write_system(&directory, "Checkout", "", &full_model()).expect("writes");
    let path = directory.join("_system.yaml");
    let document = fs::read_to_string(&path).expect("reads");
    fs::write(
        &path,
        document.replace(
            &format!("schema_version: {SCHEMA_VERSION}"),
            "schema_version: 1",
        ),
    )
    .expect("writes");

    let error = read_system(&directory).expect_err("old schema");
    assert!(error.to_string().contains("version 1"), "{error}");
}

/// A relationship to a component that is not in the design is refused.
#[test]
fn a_dangling_relationship_is_refused() {
    let directory = scratch("dangling");
    let mut model = full_model();
    model.relationships.push(Relationship {
        from: ComponentId::new("api"),
        to: ComponentId::new("absent"),
        mutators: Vec::new(),
        summary: String::new(),
    });
    let error = write_system(&directory, "Checkout", "", &model).expect_err("dangling");
    assert!(error.to_string().contains("absent"), "{error}");
}

/// An identifier that could name a path outside the directory is refused.
#[test]
fn identifiers_cannot_choose_their_own_path() {
    let directory = scratch("traversal");
    let mut model = full_model();
    model.components.push(component(
        "../../escape",
        "client",
        &[("request_rate", "1")],
    ));
    let error = write_system(&directory, "Checkout", "", &model).expect_err("unsafe identifier");
    assert!(error.to_string().contains("cannot name a file"), "{error}");
    assert!(!directory.join("../../escape.yaml").exists());
}
