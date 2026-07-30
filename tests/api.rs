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

async fn delete(address: SocketAddr, path: &str) -> u16 {
    reqwest::Client::new()
        .delete(format!("http://{address}{path}"))
        .send()
        .await
        .expect("request")
        .status()
        .as_u16()
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

/// Fetches an archive, returning the status, the disposition header, and the bytes.
async fn download(address: SocketAddr, path: &str) -> (u16, String, Vec<u8>) {
    let response = reqwest::get(format!("http://{address}{path}"))
        .await
        .expect("request");
    let status = response.status().as_u16();
    let disposition = response
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    (
        status,
        disposition,
        response.bytes().await.expect("body").to_vec(),
    )
}

async fn upload(address: SocketAddr, path: &str, archive: Vec<u8>) -> (u16, Value) {
    let response = reqwest::Client::new()
        .put(format!("http://{address}{path}"))
        .header(reqwest::header::CONTENT_TYPE, "application/zip")
        .body(archive)
        .send()
        .await
        .expect("request");
    let status = response.status().as_u16();
    (status, response.json().await.unwrap_or(Value::Null))
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

/// A remembered answer must be about the design that was asked about.
///
/// The cache exists so that flicking between variants costs nothing, which is
/// only safe if every input that changes the answer is part of what identifies
/// it. This asks the same design four ways — twice identically, then with a
/// different variant, then after an edit — and checks that only the repeat is
/// answered from memory.
#[tokio::test]
async fn a_solved_answer_is_reused_only_for_the_same_question() {
    let root = workspace("cached");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let path = "/api/v1/designs/checkout/analysis?samples=200";
    let (_, first) = get(address, path).await;
    let (_, again) = get(address, path).await;
    assert_eq!(first, again, "the same question must give the same answer");

    let (_, quieter) = get(address, &format!("{path}&intervention=quieter")).await;
    assert_ne!(
        first["components"]["api"]["offered"], quieter["components"]["api"]["offered"],
        "a variant rebinds the demand, so it cannot share the baseline's answer"
    );

    let (status, _) = post(
        address,
        "/api/v1/designs/checkout/mutations",
        json!({ "mutations": [{
            "kind": "set_scratchpad_entry",
            "entry": { "name": "peak_rate", "expression": "50", "unit": "op/s", "summary": "" }
        }] }),
    )
    .await;
    assert_eq!(status, 200);

    let (_, edited) = get(address, path).await;
    assert_eq!(edited["sequence"], 1);
    assert_ne!(
        first["components"]["api"]["offered"], edited["components"]["api"]["offered"],
        "an edit moves the design on, so the answer before it must not be reused"
    );
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

/// A design can be deleted, taking its directory with it.
///
/// An edit made moments before must not bring the design back: the write it was
/// waiting for would recreate the directory from memory, leaving a design that
/// nobody can account for.
#[tokio::test]
async fn a_design_can_be_deleted() {
    let root = workspace("delete");
    design(&root, "checkout", "Checkout");
    design(&root, "billing", "Billing");
    let address = serve(&root).await;

    let (status, _) = post(
        address,
        "/api/v1/designs/checkout/mutations",
        json!({ "mutations": [component("users")] }),
    )
    .await;
    assert_eq!(status, 200);

    assert_eq!(delete(address, "/api/v1/designs/checkout").await, 204);
    assert!(!root.join("checkout").exists());

    let (status, listing) = get(address, "/api/v1/designs").await;
    assert_eq!(status, 200);
    let ids: Vec<_> = listing
        .as_array()
        .expect("array")
        .iter()
        .map(|entry| entry["id"].as_str().expect("id").to_owned())
        .collect();
    assert_eq!(ids, ["billing"]);

    let (status, _) = get(address, "/api/v1/designs/checkout").await;
    assert_eq!(status, 404);

    // Deleting again says so rather than pretending it worked, and an identifier
    // that could name another directory never reaches the filesystem.
    assert_eq!(delete(address, "/api/v1/designs/checkout").await, 404);
    assert_eq!(delete(address, "/api/v1/designs/..%2Fescape").await, 400);
    assert!(root.join("billing").exists());
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

/// An unmatched API path stays JSON, whatever else the server is serving.
///
/// The workbench is mounted as a fallback, and a browser route has to survive a
/// reload, so anything unclaimed becomes the page. That rule must never reach
/// the API: a client receiving HTML for a mistyped endpoint would read it as a
/// malformed response to a request it believed had succeeded, rather than as
/// the 404 it is.
#[tokio::test]
async fn an_unknown_api_path_is_not_answered_with_the_workbench() {
    let root = workspace("api-404");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (status, body) = get(address, "/api/v1/nonsense").await;
    assert_eq!(status, 404);
    assert!(body["message"].is_string(), "{body}");
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

    let second: Value = serde_json::from_str(
        socket
            .next()
            .await
            .expect("message")
            .expect("frame")
            .to_text()
            .expect("text"),
    )
    .expect("json");
    assert_eq!(second["type"], "active");
    assert_eq!(second["solves"].as_array().expect("a list").len(), 0);

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
    let _active = socket.next().await.expect("message").expect("frame");

    post(
        address,
        "/api/v1/designs/checkout/mutations",
        json!({ "mutations": [component("mobile")] }),
    )
    .await;

    let quiet = tokio::time::timeout(Duration::from_millis(300), socket.next()).await;
    assert!(quiet.is_err(), "billing should hear nothing");
}

/// Two people asking the same question wait on one solve rather than two.
///
/// Counted from the feed rather than from a timer, because what matters is that
/// the work happened once and not that it happened quickly.
#[tokio::test]
async fn one_solve_answers_everyone_who_asked_for_it() {
    use futures_util::StreamExt;

    let root = workspace("solve-once");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (mut socket, _) =
        tokio_tungstenite::connect_async(format!("ws://{address}/api/v1/designs/checkout/feed"))
            .await
            .expect("connects");
    let _snapshot = socket.next().await.expect("message").expect("frame");
    let _active = socket.next().await.expect("message").expect("frame");

    let question = "/api/v1/designs/checkout/analysis?samples=8000";
    let (first, second) = tokio::join!(get(address, question), get(address, question));
    assert_eq!(first.0, 200, "{:?}", first.1);
    assert_eq!(second.0, 200, "{:?}", second.1);
    assert_eq!(first.1["components"], second.1["components"]);

    let mut announced = 0;
    let mut retired = 0;
    while let Ok(Some(Ok(frame))) =
        tokio::time::timeout(Duration::from_millis(500), socket.next()).await
    {
        let update: Value = serde_json::from_str(frame.to_text().expect("text")).expect("json");
        match update["type"].as_str() {
            Some("solving") => {
                announced += 1;
                assert_eq!(update["solve"]["kind"], "analysis");
                assert!(update["solve"]["variant"].is_null());
                assert!(update["solve"]["fraction"].as_f64().is_some(), "{update}");
                assert!(update["solve"]["steps"].as_u64().is_some(), "{update}");
            }
            Some("solved") => retired += 1,
            _ => {}
        }
    }
    assert!(announced >= 1, "the solve was never announced");
    assert_eq!(retired, 1, "the same question was solved more than once");
}

/// Somebody arriving midway through a solve is told it is already running.
#[tokio::test]
async fn a_solve_already_running_is_reported_to_whoever_arrives() {
    use futures_util::StreamExt;

    let root = workspace("solve-resume");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (mut watching, _) =
        tokio_tungstenite::connect_async(format!("ws://{address}/api/v1/designs/checkout/feed"))
            .await
            .expect("connects");
    let _snapshot = watching.next().await.expect("message").expect("frame");
    let _active = watching.next().await.expect("message").expect("frame");

    // Long enough that it is still going when the second watcher arrives.
    let asking = tokio::spawn(async move {
        get(
            address,
            "/api/v1/designs/checkout/analysis?samples=20000&horizon=60&step=0.5&transient=true",
        )
        .await
    });

    // Waited for rather than slept through, so the arrival is not a guess.
    let started = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(Ok(frame)) = watching.next().await {
            let update: Value = serde_json::from_str(frame.to_text().expect("text")).expect("json");
            if update["type"] == "solving" {
                return update;
            }
        }
        panic!("the solve was never announced");
    })
    .await
    .expect("a solve starts");
    assert_eq!(started["solve"]["kind"], "analysis");

    let (mut arriving, _) =
        tokio_tungstenite::connect_async(format!("ws://{address}/api/v1/designs/checkout/feed"))
            .await
            .expect("connects");
    let _snapshot = arriving.next().await.expect("message").expect("frame");
    let active: Value = serde_json::from_str(
        arriving
            .next()
            .await
            .expect("message")
            .expect("frame")
            .to_text()
            .expect("text"),
    )
    .expect("json");
    assert_eq!(active["type"], "active");
    let running = active["solves"].as_array().expect("a list");
    assert_eq!(running.len(), 1, "{active}");
    assert_eq!(running[0]["kind"], "analysis");
    assert_eq!(running[0]["steps"], 60);

    let (status, _) = asking.await.expect("the request finishes");
    assert_eq!(status, 200);
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

/// A design can be taken out of one workspace and put into another.
#[tokio::test]
async fn a_design_can_be_exported_and_imported_again() {
    let root = workspace("transfer");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (status, disposition, archive) =
        download(address, "/api/v1/designs/checkout/archive").await;
    assert_eq!(status, 200);
    assert!(
        disposition.contains("checkout.zip"),
        "the download is not named after the design: {disposition}"
    );

    let (status, snapshot) = upload(address, "/api/v1/designs/billing/archive", archive).await;
    assert_eq!(status, 201, "{snapshot}");
    assert_eq!(snapshot["name"], "Checkout");

    let (_, listing) = get(address, "/api/v1/designs").await;
    let ids = listing
        .as_array()
        .expect("array")
        .iter()
        .map(|design| design["id"].as_str().expect("id"))
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["billing", "checkout"]);
}

/// An export carries edits that have not yet been written to disk.
#[tokio::test]
async fn an_export_includes_unsaved_edits() {
    let root = workspace("transfer-unsaved");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (status, _) = post(
        address,
        "/api/v1/designs/checkout/mutations",
        json!({ "mutations": [component("mobile")] }),
    )
    .await;
    assert_eq!(status, 200);

    let (_, _, archive) = download(address, "/api/v1/designs/checkout/archive").await;
    let (_, snapshot) = upload(address, "/api/v1/designs/copy/archive", archive).await;
    let ids = snapshot["model"]["components"]
        .as_array()
        .expect("components")
        .iter()
        .map(|component| component["id"].as_str().expect("id"))
        .collect::<Vec<_>>();
    assert!(ids.contains(&"mobile"), "the unsaved edit did not travel");
}

/// Importing over a design is refused until somebody says to replace it.
#[tokio::test]
async fn an_import_will_not_quietly_replace_a_design() {
    let root = workspace("transfer-conflict");
    design(&root, "checkout", "Checkout");
    design(&root, "billing", "Billing");
    let address = serve(&root).await;

    let (_, _, archive) = download(address, "/api/v1/designs/checkout/archive").await;

    let (status, failure) =
        upload(address, "/api/v1/designs/billing/archive", archive.clone()).await;
    assert_eq!(status, 409);
    assert!(failure["advice"].is_string() || failure["advice"].is_array());
    // Refusing left the design it would have replaced alone.
    let (_, billing) = get(address, "/api/v1/designs/billing").await;
    assert_eq!(billing["name"], "Billing");

    let (status, snapshot) = upload(
        address,
        "/api/v1/designs/billing/archive?replace=true",
        archive,
    )
    .await;
    assert_eq!(status, 200, "{snapshot}");
    assert_eq!(snapshot["name"], "Checkout");
}

/// An upload that is not a design is refused with something to do about it.
#[tokio::test]
async fn an_upload_that_is_not_a_design_is_refused_with_advice() {
    let root = workspace("transfer-hostile");
    design(&root, "checkout", "Checkout");
    let address = serve(&root).await;

    let (status, failure) = upload(
        address,
        "/api/v1/designs/hostile/archive",
        b"this is not a zip file".to_vec(),
    )
    .await;
    assert_eq!(status, 422);
    assert!(
        failure["message"].as_str().is_some_and(|m| !m.is_empty()),
        "{failure}"
    );
    let advice = failure["advice"].as_array().expect("advice lines");
    assert!(!advice.is_empty(), "a refusal offers nothing to do");

    // Nothing was created for it.
    assert!(!root.join("hostile").exists());
    let (status, _) = get(address, "/api/v1/designs/hostile").await;
    assert_eq!(status, 404);
}
