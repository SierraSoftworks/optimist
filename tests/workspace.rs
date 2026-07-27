//! Coverage for a workspace serving several designs at once.

use std::{
    fs,
    path::{Path, PathBuf},
};

use optimist::{
    session::{DesignId, Mutation, Workspace},
    system::{Component, ComponentId},
};

fn workspace(name: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("optimist-workspace-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("scratch directory");
    path
}

fn design(root: &Path, id: &str, name: &str) {
    let directory = root.join(id);
    fs::create_dir_all(directory.join("components")).expect("design directory");
    fs::write(
        directory.join("_system.yaml"),
        format!("schema_version: 2\nname: {name}\nsummary: A design.\n"),
    )
    .expect("writes");
    fs::write(
        directory.join("components/api.yaml"),
        "id: api\nname: API\ntype: compute\nproperties:\n  service_time: '0.01'\n  parallelism: '8'\n",
    )
    .expect("writes");
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

/// Every design under the root is listed, in a stable order.
#[test]
fn designs_are_listed_by_directory_name() {
    let root = workspace("listing");
    design(&root, "checkout", "Checkout");
    design(&root, "billing", "Billing");
    // A directory that is not a design is not one.
    fs::create_dir_all(root.join("notes")).expect("directory");

    let workspace = Workspace::new(&root);
    let designs = workspace.designs().expect("lists");
    let ids = designs
        .iter()
        .map(|design| design.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["billing", "checkout"]);
    assert_eq!(designs[1].name, "Checkout");
    assert!(
        designs
            .iter()
            .all(optimist::session::DesignSummary::is_readable)
    );
}

/// A design that cannot be read is listed with the reason.
///
/// Hiding it would leave an engineer unable to discover why a design they know
/// exists has gone missing.
#[test]
fn an_unreadable_design_is_listed_with_its_error() {
    let root = workspace("broken");
    design(&root, "checkout", "Checkout");
    fs::write(
        root.join("checkout/_system.yaml"),
        "schema_version: 2\nname:\n  - not a name\n",
    )
    .expect("writes");

    let designs = Workspace::new(&root).designs().expect("lists");
    assert_eq!(designs.len(), 1);
    assert!(!designs[0].is_readable());
    assert!(designs[0].unreadable.is_some());
}

/// Everyone opening a design shares one copy of it.
///
/// The change feed only means anything if an edit made through one handle is
/// visible through another.
#[test]
fn opening_a_design_twice_shares_one_session() {
    let root = workspace("shared");
    design(&root, "checkout", "Checkout");
    let workspace = Workspace::new(&root);
    let id = DesignId::new("checkout").expect("identifier");

    let first = workspace.session(&id).expect("opens");
    let second = workspace.session(&id).expect("opens");
    let mut watching = second.watch();

    let sequence = first
        .apply(Mutation::SetComponent {
            component: component("users"),
        })
        .expect("applies");

    assert_eq!(second.snapshot().sequence, sequence);
    assert!(
        second
            .snapshot()
            .model
            .components
            .iter()
            .any(|component| component.id.as_str() == "users")
    );
    let change = watching.try_recv().expect("change delivered");
    assert_eq!(change.sequence, sequence);
}

/// Designs are independent of each other.
#[test]
fn editing_one_design_does_not_disturb_another() {
    let root = workspace("independent");
    design(&root, "checkout", "Checkout");
    design(&root, "billing", "Billing");
    let workspace = Workspace::new(&root);

    let checkout = workspace
        .session(&DesignId::new("checkout").expect("identifier"))
        .expect("opens");
    let billing = workspace
        .session(&DesignId::new("billing").expect("identifier"))
        .expect("opens");
    let mut watching = billing.watch();

    checkout
        .apply(Mutation::SetComponent {
            component: component("users"),
        })
        .expect("applies");

    assert_eq!(billing.snapshot().sequence, 0);
    assert_eq!(billing.snapshot().model.components.len(), 1);
    assert!(watching.try_recv().is_err(), "billing must hear nothing");
}

/// Asking for a design that is not there says so.
#[test]
fn an_absent_design_is_reported() {
    let root = workspace("absent");
    design(&root, "checkout", "Checkout");
    let workspace = Workspace::new(&root);
    let Err(error) = workspace.session(&DesignId::new("missing").expect("identifier")) else {
        panic!("an absent design should not open");
    };
    assert!(error.to_string().contains("no design named"), "{error}");
}

/// An identifier cannot name a directory outside the workspace.
#[test]
fn identifiers_cannot_escape_the_workspace() {
    for value in ["..", "../elsewhere", "a/b", "/etc"] {
        assert!(DesignId::new(value).is_err(), "'{value}' should be refused");
    }
}

/// Only loaded designs are written, and only where they have unsaved edits.
#[test]
fn persisting_covers_the_designs_in_use() {
    let root = workspace("persist");
    design(&root, "checkout", "Checkout");
    design(&root, "billing", "Billing");
    let workspace = Workspace::new(&root);

    // Nothing has been opened, so there is nothing to write.
    assert_eq!(workspace.persist_all().expect("persists"), 0);

    let checkout = workspace
        .session(&DesignId::new("checkout").expect("identifier"))
        .expect("opens");
    checkout
        .apply(Mutation::SetComponent {
            component: component("users"),
        })
        .expect("applies");

    assert_eq!(workspace.persist_all().expect("persists"), 1);
    assert!(root.join("checkout/components/users.yaml").exists());
    // Writing again has nothing left to do.
    assert_eq!(workspace.persist_all().expect("persists"), 0);
}

/// A design written by one handle is on disk for the next process to read.
#[test]
fn edits_survive_reopening_the_workspace() {
    let root = workspace("reopen");
    design(&root, "checkout", "Checkout");

    let workspace = Workspace::new(&root);
    let id = DesignId::new("checkout").expect("identifier");
    workspace
        .session(&id)
        .expect("opens")
        .apply(Mutation::SetComponent {
            component: component("users"),
        })
        .expect("applies");
    workspace.persist_all().expect("persists");

    let reopened = Workspace::new(&root);
    let model = reopened.session(&id).expect("opens").snapshot().model;
    assert!(
        model
            .components
            .iter()
            .any(|component| component.id.as_str() == "users")
    );
}
