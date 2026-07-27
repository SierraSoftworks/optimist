//! Coverage for the HTTP and WebSocket surface.
//!
//! The tests drive a real server over a real socket, because the parts worth
//! checking are the ones that only exist once a request has been routed: how a
//! change reaches a second client, and what a client is told when it asks for
//! something that is not there.

use std::{fs, net::SocketAddr, path::Path, sync::Arc, time::Duration};

use optimist::{api::router, session::Workspace};
use serde_json::{Value, json};
use tokio::net::TcpListener;

fn workspace(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("optimist-api-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("scratch directory");
    path
}

fn design(root: &Path, id: &str, name: &str) {
    let directory = root.join(id);
    fs::create_dir_all(directory.join("components")).expect("design directory");
    fs::write(
        directory.join("_system.yaml"),
        format!(
            "schema_version: 2\nname: {name}\nsummary: A design.\n\
             scratchpad:\n- name: peak_rate\n  expression: '900'\n  unit: op/s\n  summary: ''\n\
             interventions:\n- id: quieter\n  name: Quieter\n  summary: ''\n  \
             overrides:\n  - name: peak_rate\n    expression: '100'\n",
        ),
    )
    .expect("writes");
    fs::write(
        directory.join("components/users.yaml"),
        "id: users\nname: Users\ntype: client\nproperties:\n  request_rate: peak_rate\noutgoing:\n- to: api\n",
    )
    .expect("writes");
    fs::write(
        directory.join("components/api.yaml"),
        "id: api\nname: API\ntype: compute\nproperties:\n  service_time: '0.02'\n  parallelism: '8'\n",
    )
    .expect("writes");
}

/// Starts a server on an ephemeral port and returns where to reach it.
async fn serve(root: &Path) -> SocketAddr {
    let workspace = Arc::new(Workspace::new(root));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let address = listener.local_addr().expect("address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router(workspace)).await;
    });
    address
}

async fn get(address: SocketAddr, path: &str) -> (u16, Value) {
    let response = reqwest::get(format!("http://{address}{path}"))
        .await
        .expect("request");
    let status = response.status().as_u16();
    let body = response.json().await.unwrap_or(Value::Null);
    (status, body)
}

async fn post(address: SocketAddr, path: &str, body: Value) -> (u16, Value) {
    let response = reqwest::Client::new()
        .post(format!("http://{address}{path}"))
        .json(&body)
        .send()
        .await
        .expect("request");
    let status = response.status().as_u16();
    let body = response.json().await.unwrap_or(Value::Null);
    (status, body)
}

fn component(id: &str) -> Value {
    json!({
        "kind": "set_component",
        "component": {
            "id": id,
            "name": id,
            "type": "client",
            "properties": { "request_rate": "10" }
        }
    })
}

/// The workspace lists what it serves, and a design can be read from it.
#[tokio::test]
async fn designs_can_be_listed_and_read() {
    let root = workspace("listing");
    design(&root, "checkout", "Checkout");
    design(&root, "billing", "Billing");
    let address = serve(&root).await;

    let (status, listing) = get(address, "/api/v1/designs").await;
    assert_eq!(status, 200);
    let ids = listing
        .as_array()
        .expect("array")
        .iter()
        .map(|design| design["id"].as_str().expect("id"))
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["billing", "checkout"]);

    let (status, design) = get(address, "/api/v1/designs/checkout").await;
    assert_eq!(status, 200);
    assert_eq!(design["name"], "Checkout");
    assert_eq!(design["sequence"], 0);
    assert_eq!(
        design["model"]["components"]
            .as_array()
            .expect("array")
            .len(),
        2
    );
}

/// A design that is not there is a not-found, with advice rather than a bare code.
#[tokio::test]
async fn an_absent_design_is_reported_with_advice() {
    let root = workspace("absent");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (status, failure) = get(address, "/api/v1/designs/missing").await;
    assert_eq!(status, 404);
    assert!(
        failure["message"]
            .as_str()
            .expect("message")
            .contains("missing"),
        "{failure}"
    );
    assert!(failure["advice"].is_string(), "{failure}");
}

/// A path that could name another directory is refused before it is used.
#[tokio::test]
async fn a_traversing_identifier_is_refused() {
    let root = workspace("traversal");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (status, _) = get(address, "/api/v1/designs/..%2F..%2Fetc").await;
    assert!(matches!(status, 400 | 404), "unexpected status {status}");
}

/// A change applies without a revision being sent or returned.
#[tokio::test]
async fn a_change_applies_without_a_revision() {
    let root = workspace("mutate");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (status, applied) = post(
        address,
        "/api/v1/designs/checkout/mutations",
        json!({ "mutations": [component("mobile")] }),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(applied["applied"], 1);
    assert_eq!(applied["sequence"], 1);

    let (_, design) = get(address, "/api/v1/designs/checkout").await;
    assert_eq!(
        design["model"]["components"]
            .as_array()
            .expect("array")
            .len(),
        3
    );
}

/// Several changes apply in the order they were sent.
#[tokio::test]
async fn changes_apply_in_order() {
    let root = workspace("batch");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (_, applied) = post(
        address,
        "/api/v1/designs/checkout/mutations",
        json!({ "mutations": [component("one"), component("two"), component("three")] }),
    )
    .await;
    assert_eq!(applied["applied"], 3);
    assert_eq!(applied["sequence"], 3);
}

/// A change that would break the design is refused and says why.
#[tokio::test]
async fn a_structurally_broken_change_is_refused() {
    let root = workspace("broken");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (status, failure) = post(
        address,
        "/api/v1/designs/checkout/mutations",
        json!({
            "mutations": [{
                "kind": "set_relationship",
                "relationship": { "from": "users", "to": "nowhere", "mutators": [], "summary": "" }
            }]
        }),
    )
    .await;
    assert_eq!(status, 409);
    assert!(
        failure["message"]
            .as_str()
            .expect("message")
            .contains("nowhere"),
        "{failure}"
    );
}

/// Solving reports the constraints and the position it reflects.
#[tokio::test]
async fn a_design_can_be_solved_over_http() {
    let root = workspace("analysis");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (status, analysis) = get(address, "/api/v1/designs/checkout/analysis?samples=200").await;
    assert_eq!(status, 200);
    assert_eq!(analysis["converged"], true);
    assert_eq!(analysis["sequence"], 0);

    // Eight slots at 20 ms sustain 400 per second against 900 offered.
    let worst = &analysis["bottlenecks"][0];
    assert_eq!(worst["component"], "api");
    assert_eq!(worst["constraint"], "capacity");
    assert!(worst["probability_of_binding"].as_f64().expect("number") > 0.9);
}

/// Solving carries the draws behind each quantity, not only its summary.
///
/// A ranking says how loaded something is; the draws say whether the load is one
/// thing or two. A design that has settled on two branches has a spread that no
/// mean, percentile pair, or interval can distinguish from a single wide one, so
/// the shape has to reach the client intact for a chart to tell the difference.
#[tokio::test]
async fn solving_returns_the_draws_behind_each_quantity() {
    let root = workspace("draws");
    let directory = root.join("uncertain");
    fs::create_dir_all(directory.join("components")).expect("design directory");
    fs::write(
        directory.join("_system.yaml"),
        "schema_version: 2\nname: Uncertain\nsummary: A design.\n\
         scratchpad:\n- name: peak_rate\n  expression: 900 * lognormal(0, 0.4)\n  \
         unit: op/s\n  summary: ''\n",
    )
    .expect("writes");
    fs::write(
        directory.join("components/users.yaml"),
        "id: users\nname: Users\ntype: client\nproperties:\n  request_rate: peak_rate\noutgoing:\n- to: api\n",
    )
    .expect("writes");
    fs::write(
        directory.join("components/api.yaml"),
        "id: api\nname: API\ntype: compute\nproperties:\n  service_time: '0.02'\n  parallelism: '8'\n",
    )
    .expect("writes");
    let address = serve(&root).await;

    let (status, analysis) = get(address, "/api/v1/designs/uncertain/analysis?samples=2000").await;
    assert_eq!(status, 200);

    let offered = &analysis["components"]["api"]["offered"];
    let draws = offered["draws"].as_array().expect("draws");
    assert!(
        !draws.is_empty() && draws.len() <= 256,
        "draws must arrive and stay within the budget, got {}",
        draws.len()
    );
    assert!(
        draws.iter().any(|draw| draw != &draws[0]),
        "an uncertain quantity must not arrive as one repeated value"
    );
    assert!(
        offered["p10"].as_f64().expect("number") < offered["p90"].as_f64().expect("number"),
        "and its summary must bracket that spread"
    );

    // Service time and parallelism are certain, so capacity is too. Sending no
    // draws is how a client is told to render a point rather than a spread.
    let capacity = &analysis["components"]["api"]["capacity"];
    assert_eq!(
        capacity["draws"].as_array().expect("draws").len(),
        0,
        "a certain quantity carries no draws"
    );
    assert_eq!(capacity["mean"], 400.0);
}

/// A design can be started empty and edited into existence.
///
/// The first thing anybody does is name the system they are about to model, so
/// a design with nothing in it has to be storable. Requiring a component before
/// a design can exist would mean the first edit had to carry the creation too.
#[tokio::test]
async fn a_design_can_be_created_and_then_edited() {
    let root = workspace("create");
    let address = serve(&root).await;

    let (status, created) = post(
        address,
        "/api/v1/designs",
        json!({ "id": "ledger", "name": "Ledger", "summary": "Books." }),
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(created["name"], "Ledger");
    assert_eq!(
        created["model"]["components"]
            .as_array()
            .expect("array")
            .len(),
        0
    );

    let (status, listing) = get(address, "/api/v1/designs").await;
    assert_eq!(status, 200);
    assert_eq!(listing.as_array().expect("array").len(), 1);

    let (status, applied) = post(
        address,
        "/api/v1/designs/ledger/mutations",
        json!({ "mutations": [component("users")] }),
    )
    .await;
    assert_eq!(status, 200, "{applied}");
    assert_eq!(applied["applied"], 1);
}

/// Two designs cannot share an identifier.
///
/// The identifier is a directory name, so accepting a repeat would overwrite
/// somebody else's design. A conflict is the safe answer.
#[tokio::test]
async fn a_design_cannot_be_created_twice() {
    let root = workspace("create-twice");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (status, failure) = post(address, "/api/v1/designs", json!({ "id": "checkout" })).await;
    assert_eq!(status, 409);
    assert!(
        failure["message"]
            .as_str()
            .expect("message")
            .contains("checkout"),
        "{failure}"
    );

    // A name that could climb out of the workspace is refused before it reaches
    // the filesystem.
    let (status, _) = post(address, "/api/v1/designs", json!({ "id": "../escape" })).await;
    assert_eq!(status, 400);
}

/// Solving can report every step, which is what a chart over time needs.
#[tokio::test]
async fn a_series_is_returned_only_when_it_is_asked_for() {
    let root = workspace("series");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (_, quiet) = get(
        address,
        "/api/v1/designs/checkout/analysis?samples=128&horizon=4",
    )
    .await;
    assert!(
        quiet["series"].is_null(),
        "a series costs a horizon's worth of draws"
    );

    let (status, full) = get(
        address,
        "/api/v1/designs/checkout/analysis?samples=128&horizon=4&series=true",
    )
    .await;
    assert_eq!(status, 200);
    let steps = full["series"].as_array().expect("series");
    assert_eq!(steps.len(), 4, "one frame per step");
    assert_eq!(steps[0]["time"], 0.0);
    assert!(steps[0]["components"]["api"]["offered"]["mean"].is_number());
}

/// The catalogue carries the language's vocabulary for the editor to complete.
#[tokio::test]
async fn the_catalogue_lists_what_an_expression_may_call() {
    let root = workspace("builtins");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (status, catalogue) = get(address, "/api/v1/designs/checkout/catalogue").await;
    assert_eq!(status, 200);
    let builtins = catalogue["builtins"].as_array().expect("builtins");
    assert!(builtins.iter().any(|name| name == "Little.occupancy"));
    assert!(builtins.iter().any(|name| name == "normal"));
}

/// A proposal is weighed against the design over the same surface.
#[tokio::test]
async fn a_proposal_can_be_compared_over_http() {
    let root = workspace("comparison");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (status, comparison) = get(
        address,
        "/api/v1/designs/checkout/comparisons/quieter?samples=200",
    )
    .await;
    assert_eq!(status, 200);
    assert!(
        comparison["baseline"][0]["probability_of_binding"]
            .as_f64()
            .expect("number")
            > 0.9
    );
    assert_eq!(comparison["proposed"][0]["probability_of_binding"], 0.0);
}

/// Asking for an intervention the design does not declare is reported.
#[tokio::test]
async fn an_unknown_proposal_is_reported() {
    let root = workspace("unknown-proposal");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (status, failure) = get(address, "/api/v1/designs/checkout/comparisons/imaginary").await;
    assert_eq!(status, 422);
    assert!(
        failure["message"]
            .as_str()
            .expect("message")
            .contains("imaginary"),
        "{failure}"
    );
}

/// The feed opens with the design and then carries the changes made to it.
///
/// The mutation is delivered rather than a new design, so a client applies the
/// same change the server did and leaves whatever it is editing alone.
#[tokio::test]
async fn the_feed_opens_with_a_snapshot_and_streams_mutations() {
    use futures_util::StreamExt;

    let root = workspace("feed");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (mut socket, _) =
        tokio_tungstenite::connect_async(format!("ws://{address}/api/v1/designs/checkout/feed"))
            .await
            .expect("connects");

    let first: Value = serde_json::from_str(
        socket
            .next()
            .await
            .expect("message")
            .expect("frame")
            .to_text()
            .expect("text"),
    )
    .expect("json");
    assert_eq!(first["type"], "snapshot");
    assert_eq!(first["sequence"], 0);
    assert_eq!(first["name"], "Checkout");

    post(
        address,
        "/api/v1/designs/checkout/mutations",
        json!({ "mutations": [component("mobile")] }),
    )
    .await;

    let next = tokio::time::timeout(Duration::from_secs(5), socket.next())
        .await
        .expect("a change arrives")
        .expect("message")
        .expect("frame");
    let change: Value = serde_json::from_str(next.to_text().expect("text")).expect("json");
    assert_eq!(change["type"], "change");
    assert_eq!(change["sequence"], 1);
    // The mutation itself is carried, not a replacement design.
    assert_eq!(change["mutation"]["kind"], "set_component");
    assert_eq!(change["mutation"]["component"]["id"], "mobile");
    assert!(
        change.get("model").is_none(),
        "the feed must not resend the design"
    );
}

/// Watchers of one design hear nothing about another.
#[tokio::test]
async fn the_feed_is_scoped_to_its_design() {
    use futures_util::StreamExt;

    let root = workspace("feed-scope");
    design(&root, "checkout", "Checkout");
    design(&root, "billing", "Billing");
    let address = serve(&root).await;

    let (mut socket, _) =
        tokio_tungstenite::connect_async(format!("ws://{address}/api/v1/designs/billing/feed"))
            .await
            .expect("connects");
    let _snapshot = socket.next().await.expect("message").expect("frame");

    post(
        address,
        "/api/v1/designs/checkout/mutations",
        json!({ "mutations": [component("mobile")] }),
    )
    .await;

    let quiet = tokio::time::timeout(Duration::from_millis(300), socket.next()).await;
    assert!(quiet.is_err(), "billing should hear nothing");
}

/// Edits reach disk without anyone asking for them to.
#[tokio::test]
async fn edits_are_written_after_they_settle() {
    let root = workspace("persist");
    design(&root, "checkout", "Checkout");
    let workspace_handle = Arc::new(Workspace::new(&root));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let address = listener.local_addr().expect("address");
    let serving = Arc::clone(&workspace_handle);
    tokio::spawn(async move {
        let _ = axum::serve(listener, router(serving)).await;
    });

    post(
        address,
        "/api/v1/designs/checkout/mutations",
        json!({ "mutations": [component("mobile")] }),
    )
    .await;

    // The sweep runs inside `serve`; this exercises the same call it makes.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let written = tokio::task::spawn_blocking(move || workspace_handle.persist_due())
        .await
        .expect("sweeps")
        .expect("writes");
    assert_eq!(written, 1);
    assert!(root.join("checkout/components/mobile.yaml").exists());
}

/// Health reports what the process is holding.
#[tokio::test]
async fn health_reports_loaded_designs() {
    let root = workspace("health");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (status, health) = get(address, "/api/v1/health").await;
    assert_eq!(status, 200);
    assert_eq!(health["status"], "ok");
    assert_eq!(health["designs"], 0);

    get(address, "/api/v1/designs/checkout").await;
    let (_, health) = get(address, "/api/v1/health").await;
    assert_eq!(health["designs"], 1);
}
