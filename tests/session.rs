//! Coverage for the in-memory design a server shares between editors.

use std::{fs, path::PathBuf};

use optimist::{
    session::{Mutation, Session},
    system::{Component, ComponentId, Relationship, ScaleUnit, ScaleUnitId, ScratchpadEntry},
};

fn design(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("optimist-session-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(path.join("components")).expect("scratch directory");
    fs::write(
        path.join("_system.yaml"),
        "schema_version: 2\nname: Checkout\nsummary: ''\n",
    )
    .expect("writes");
    for (id, body) in [
        (
            "users",
            "id: users\nname: Users\ntype: client\nproperties:\n  request_rate: '100'\noutgoing:\n- to: api\n",
        ),
        (
            "api",
            "id: api\nname: API\ntype: compute\nproperties:\n  service_time: '0.01'\n  parallelism: '8'\n",
        ),
    ] {
        fs::write(path.join(format!("components/{id}.yaml")), body).expect("writes");
    }
    path
}

fn component(id: &str) -> Component {
    Component {
        id: ComponentId::new(id),
        name: id.to_owned(),
        component_type: serde_yaml_ng::from_str("client").expect("type"),
        properties: [("request_rate".to_owned(), "10".to_owned())]
            .into_iter()
            .collect(),
        position: None,
    }
}

fn shared(name: &str, expression: &str) -> Mutation {
    Mutation::SetScratchpadEntry {
        entry: ScratchpadEntry {
            name: name.to_owned(),
            expression: expression.to_owned(),
            unit: None,
            summary: String::new(),
        },
    }
}

/// A session opens the design it was pointed at.
#[test]
fn a_session_opens_a_design() {
    let session = Session::open(&design("open")).expect("opens");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.name, "Checkout");
    assert_eq!(snapshot.model.components.len(), 2);
    assert_eq!(snapshot.sequence, 0);
}

/// Each applied change advances the feed by one.
#[test]
fn changes_advance_the_feed() {
    let session = Session::open(&design("feed")).expect("opens");
    assert_eq!(session.apply(shared("peak", "100")).expect("applies"), 1);
    assert_eq!(session.apply(shared("size", "512")).expect("applies"), 2);
    assert_eq!(session.snapshot().sequence, 2);
}

/// Everyone watching sees a change as it is applied.
///
/// This is what makes last-write-wins tolerable: a stale read lasts as long as
/// it takes to deliver a message, rather than as long as someone leaves a form
/// open.
#[test]
fn watchers_are_told_about_changes() {
    let session = Session::open(&design("watch")).expect("opens");
    let mut first = session.watch();
    let mut second = session.watch();

    session.apply(shared("peak", "100")).expect("applies");

    for listener in [&mut first, &mut second] {
        let change = listener.try_recv().expect("receives");
        assert_eq!(change.sequence, 1);
        assert!(matches!(
            change.mutation,
            Mutation::SetScratchpadEntry { .. }
        ));
    }
}

/// A later write to the same thing replaces the earlier one.
#[test]
fn the_last_writer_wins() {
    let session = Session::open(&design("last-write")).expect("opens");
    session.apply(shared("peak", "100")).expect("applies");
    session.apply(shared("peak", "900")).expect("applies");

    let scratchpad = session.snapshot().model.scratchpad;
    assert_eq!(scratchpad.len(), 1, "the entry is replaced, not duplicated");
    assert_eq!(scratchpad[0].expression, "900");
}

/// Edits to different things do not contend at all.
#[test]
fn separate_entities_never_conflict() {
    let session = Session::open(&design("independent")).expect("opens");
    session.apply(shared("peak", "100")).expect("applies");
    session
        .apply(Mutation::SetComponent {
            component: component("mobile"),
        })
        .expect("applies");

    let snapshot = session.snapshot();
    assert_eq!(snapshot.model.scratchpad.len(), 1);
    assert_eq!(snapshot.model.components.len(), 3);
}

/// Removing a component takes its connections with it.
///
/// A connection to something that is gone would make the design unreadable, so
/// the removal is complete rather than leaving the author to find the pieces.
#[test]
fn removing_a_component_removes_its_connections() {
    let session = Session::open(&design("cascade")).expect("opens");
    assert_eq!(session.snapshot().model.relationships.len(), 1);

    session
        .apply(Mutation::RemoveComponent {
            id: ComponentId::new("api"),
        })
        .expect("applies");

    let snapshot = session.snapshot();
    assert_eq!(snapshot.model.components.len(), 1);
    assert!(snapshot.model.relationships.is_empty());
}

/// A change that would break the design is refused rather than half-applied.
#[test]
fn structurally_invalid_changes_are_refused() {
    let session = Session::open(&design("invalid")).expect("opens");

    let dangling = session.apply(Mutation::SetRelationship {
        relationship: Relationship {
            from: ComponentId::new("api"),
            to: ComponentId::new("absent"),
            mutators: Vec::new(),
            summary: String::new(),
        },
    });
    assert!(dangling.is_err());

    let loop_back = session.apply(Mutation::SetRelationship {
        relationship: Relationship {
            from: ComponentId::new("api"),
            to: ComponentId::new("api"),
            mutators: Vec::new(),
            summary: String::new(),
        },
    });
    assert!(loop_back.is_err());

    // A refused change does not advance the feed.
    assert_eq!(session.snapshot().sequence, 0);
}

/// An incomplete design is accepted, because that is what editing looks like.
#[test]
fn an_incomplete_design_is_still_editable() {
    let session = Session::open(&design("incomplete")).expect("opens");
    let mut bare = component("draft");
    bare.properties.clear();
    session
        .apply(Mutation::SetComponent { component: bare })
        .expect("a component missing its properties is mid-edit, not invalid");
}

/// A scale unit cannot claim a component another already holds.
#[test]
fn scale_units_cannot_share_a_component() {
    let session = Session::open(&design("membership")).expect("opens");
    let unit = |id: &str| ScaleUnit {
        id: ScaleUnitId::new(id),
        name: id.to_owned(),
        summary: String::new(),
        replicas: "2".to_owned(),
        distribution: optimist::system::Distribution::Sharded,
        members: vec![ComponentId::new("api")],
        parent: None,
    };
    session
        .apply(Mutation::SetScaleUnit {
            scale_unit: unit("left"),
        })
        .expect("applies");
    assert!(
        session
            .apply(Mutation::SetScaleUnit {
                scale_unit: unit("right"),
            })
            .is_err()
    );
}

/// Removing something that is not there is reported rather than ignored.
#[test]
fn removing_something_absent_is_reported() {
    let session = Session::open(&design("absent")).expect("opens");
    let error = session
        .apply(Mutation::RemoveScratchpadEntry {
            name: "never_existed".to_owned(),
        })
        .expect_err("absent");
    assert!(error.to_string().contains("never_existed"), "{error}");
}

/// Edits reach disk, and what lands there is a complete readable design.
#[test]
fn edits_are_written_as_a_whole_snapshot() {
    let directory = design("persist");
    let session = Session::open(&directory).expect("opens");
    session.apply(shared("peak", "900")).expect("applies");
    assert!(session.pending(), "an edit is not yet on disk");

    session.persist().expect("writes");
    assert!(!session.pending());

    // Reopening reads exactly what was in memory, with no journal to replay.
    let reopened = Session::open(&directory).expect("reopens");
    assert_eq!(reopened.snapshot().model.scratchpad[0].expression, "900");
}

/// Writing waits for a pause so a burst of edits costs one write.
#[test]
fn writing_waits_for_the_design_to_settle() {
    let session = Session::open(&design("debounce")).expect("opens");
    session.apply(shared("peak", "100")).expect("applies");
    assert!(
        !session.persist_if_due().expect("checks"),
        "a change made just now has not settled"
    );
    assert!(session.pending());
}
