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
        position: None,
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
                from_port: None,
                to_port: None,
                capacity: None,
                bandwidth: None,
                latency: None,
                from: ComponentId::new("users"),
                to: ComponentId::new("api"),
                mutators: vec![AttachedMutator {
                    mutator: MutatorId::new("retry"),
                    properties: [("attempts".to_owned(), "3".to_owned())]
                        .into_iter()
                        .collect(),
                }],
                summary: "Callers reaching the entry point.".to_owned(),
            },
            Relationship {
                from_port: None,
                to_port: None,
                capacity: None,
                bandwidth: None,
                latency: None,
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

/// Where a component was placed on the diagram is part of the design.
///
/// Somebody arranging a diagram is saying how the system is best read, and that
/// judgement is worth keeping and worth reviewing beside the model it describes.
/// A component nobody has placed carries no position at all, so a design that
/// has never been arranged is laid out automatically rather than pinned to
/// whatever an algorithm produced the first time it was opened.
#[test]
fn a_placement_is_stored_with_the_design() {
    let directory = scratch("placement");
    let mut model = full_model();
    model.components[0].position = Some(optimist::system::Position { x: 321.0, y: 123.0 });
    write_system(&directory, "Checkout", "", &model).expect("writes");

    let placed = model.components[0].id.clone();
    let loaded = read_system(&directory).expect("reads");
    let component = loaded
        .model
        .components
        .iter()
        .find(|component| component.id == placed)
        .expect("the placed component");
    let position = component.position.expect("a position");
    assert_eq!((position.x, position.y), (321.0, 123.0));

    assert!(
        loaded
            .model
            .components
            .iter()
            .any(|component| component.position.is_none()),
        "a component nobody moved must not acquire a position"
    );
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
            evaluation.settled().components[&api].channels["rate"]
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
        from_port: None,
        to_port: None,
        capacity: None,
        bandwidth: None,
        latency: None,
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
         \x20   expression: min([in.requests.rate, ceiling])\n\
         \x20 admitted_ratio:\n\
         \x20   unit: share\n\
         \x20   summary: Share of calls the cap let through.\n\
         \x20   expression: min([admitted / max([in.requests.rate, 0.000001]), 1])\n\
         ports:\n\
         \x20 in:\n\
         \x20   requests:\n\
         \x20     publishes:\n\
         \x20       capacity: ceiling\n\
         \x20       latency: '0'\n\
         \x20       success: admitted_ratio\n\
         \x20 out:\n\
         \x20   onward:\n\
         \x20     publishes:\n\
         \x20       rate: admitted\n\
         constraints:\n\
         \x20 ceiling:\n\
         \x20   summary: Demand against the cap.\n\
         \x20   demand: in.requests.rate\n\
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

/// Strictness reaches inside a document rather than stopping at its top level.
///
/// A design-wide document is a list of entries, and an entry is where an author
/// actually writes. Refusing an unknown key beside `name` while accepting one
/// inside the entry below it would guard the part nobody mistypes.
#[test]
fn unknown_fields_are_refused_inside_a_document() {
    let directory = scratch("strict-nested");
    write_system(&directory, "Checkout", "", &full_model()).expect("writes");
    fs::write(
        directory.join("_system.yaml"),
        format!(
            "schema_version: {SCHEMA_VERSION}\n\
             name: Checkout\n\
             scratchpad:\n\
             \x20 - name: peak_rate\n\
             \x20   expression: lognormal(6, 0.3)\n\
             \x20   unti: op/s\n"
        ),
    )
    .expect("writes");

    let error = read_system(&directory).expect_err("unknown field");
    assert!(error.to_string().contains("unti"), "{error}");
}

/// A component type manifest is held to the same rule as a design document.
///
/// This is where a key that has been renamed does the most damage. A manifest
/// naming a section the engine no longer reads produces a type with no ports,
/// which solves, reports numbers, and is wrong everywhere the missing section
/// would have carried a flow.
#[test]
fn a_manifest_naming_an_absent_section_is_refused() {
    let directory = scratch("strict-manifest");
    write_system(&directory, "Checkout", "", &full_model()).expect("writes");
    fs::create_dir_all(directory.join("component-types")).expect("directory");
    fs::write(
        directory.join("component-types/legacy.yaml"),
        "id: legacy\n\
         name: Legacy\n\
         properties:\n\
         \x20 refill:\n\
         \x20   unit: op/s\n\
         channels:\n\
         \x20 admitted:\n\
         \x20   unit: op/s\n\
         \x20   expression: min([in.requests.rate, refill])\n\
         outputs:\n\
         \x20 rate: admitted\n",
    )
    .expect("writes");

    let error = read_system(&directory).expect_err("unknown section");
    assert!(error.to_string().contains("outputs"), "{error}");
    assert!(error.to_string().contains("ports"), "{error}");
}

/// A behaviour manifest is held to the same rule as everything else.
#[test]
fn a_behaviour_manifest_with_an_unknown_field_is_refused() {
    let directory = scratch("strict-behaviour");
    write_system(&directory, "Checkout", "", &full_model()).expect("writes");
    fs::create_dir_all(directory.join("mutators")).expect("directory");
    fs::write(
        directory.join("mutators/sample.yaml"),
        "id: sample\n\
         name: Sampling\n\
         properties:\n\
         \x20 ratio:\n\
         \x20   unit: '1'\n\
         requests:\n\
         \x20 rate:\n\
         \x20   unit: op/s\n\
         \x20   expresion: signal.rate * ratio\n",
    )
    .expect("writes");

    let error = read_system(&directory).expect_err("unknown field");
    assert!(error.to_string().contains("expresion"), "{error}");
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
        from_port: None,
        to_port: None,
        capacity: None,
        bandwidth: None,
        latency: None,
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
