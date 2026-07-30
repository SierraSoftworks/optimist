//! What a quorum buys, and what it costs, at the boundaries of the arrangement.
//!
//! A quorum is the one component in the catalogue whose reliability *rises* with
//! the number of dependencies it has, and whose latency *falls*. Both readings
//! come from the same fact — that a majority leaves the slowest and the failed
//! behind — and both are easy to state and hard to believe, so each is checked
//! here against the closed form rather than against a recorded number.
//!
//! The group's size is not authored. It is read from the deployment through the
//! `peers` signal, which is the engine telling a component how many replicas of
//! its neighbour it is talking to. The tests that matter most are therefore the
//! ones about where that count comes from: a scale unit around the member, a
//! scale unit around both ends, and no scale unit at all.

use std::collections::BTreeMap;

use optimist::squiggle::{Runtime, Value};
use optimist::system::{EvaluationConfig, SystemModel, builtin_catalogue, evaluate};

fn config() -> EvaluationConfig {
    EvaluationConfig {
        seed: 7,
        sample_count: 200,
        ..EvaluationConfig::default()
    }
}

fn mean(value: &Value) -> f64 {
    match value {
        Value::Number(number) => *number,
        Value::Distribution(distribution) => distribution.mean().expect("mean"),
        other => panic!("expected a quantity, got {other:?}"),
    }
}

type Channels = BTreeMap<String, BTreeMap<String, Value>>;

fn try_solve(source: &str) -> Result<Channels, String> {
    let model: SystemModel = serde_yaml_ng::from_str(source).expect("the model parses");
    let catalogue = builtin_catalogue().expect("catalogue");
    let evaluation = evaluate(&model, &catalogue, config())
        .map_err(|error: optimist::system::EvaluationError| error.to_string())?;
    assert!(evaluation.converged(), "the model did not settle");
    Ok(evaluation
        .settled()
        .components
        .iter()
        .map(|(id, state)| (id.to_string(), state.channels.clone()))
        .collect())
}

fn solve(source: &str) -> Channels {
    match try_solve(source) {
        Ok(channels) => channels,
        Err(error) => panic!("expected the model to solve: {error}"),
    }
}

#[track_caller]
fn get(channels: &Channels, component: &str, channel: &str) -> f64 {
    let owned = channels
        .get(component)
        .unwrap_or_else(|| panic!("no component '{component}'"));
    mean(
        owned
            .get(channel)
            .unwrap_or_else(|| panic!("no channel '{component}.{channel}'")),
    )
}

#[track_caller]
fn close(actual: f64, expected: f64, what: &str) {
    let tolerance = expected.abs().max(1.0) * 1e-6;
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: expected {expected}, got {actual}"
    );
}

/// Evaluates a squiggle expression to the number it produces.
#[track_caller]
fn number(source: &str) -> f64 {
    match Runtime::new().evaluate(source).expect("the law evaluates") {
        Value::Number(number) => number,
        other => panic!("expected a number from '{source}', got {other:?}"),
    }
}

/// A quorum in front of one member, optionally replicated by a scale unit.
///
/// The member is deployed mirrored, because every node of a quorum receives
/// every request: sharding it would describe a group that divides its work,
/// which is a fan-out with extra steps rather than a quorum.
fn grouped(replicas: Option<&str>) -> String {
    fallible(replicas, "0")
}

/// The same group, with each node losing a share of its replies.
///
/// Availability arithmetic needs something to be unavailable, and a compute pool
/// inside its capacity never fails, so the failure is put on the wire where it
/// can be dialled. Reply loss rather than request loss, so that the load the
/// group places on its nodes is unchanged and the two readings stay separable.
fn fallible(replicas: Option<&str>, receive_failure: &str) -> String {
    let unit = match replicas {
        Some(replicas) => format!(
            "
scale_units:
  - id: ring
    name: Ring
    replicas: '{replicas}'
    distribution: mirrored
    members: [node]
"
        ),
        None => String::new(),
    };
    format!(
        "
components:
  - id: users
    name: Users
    type: client
    properties:
      request_rate: '100'
      latency_target: '1'
      success_target: '0.99'
  - id: group
    name: Group
    type: quorum
    properties:
      overhead: '0.001'
  - id: node
    name: Node
    type: compute
    properties:
      service_time: '0.02'
      parallelism: '400'
relationships:
  - from: users
    to: group
  - from: group
    to: node
    mutators:
      - type: fallible
        properties:
          receive_failure: '{receive_failure}'
{unit}"
    )
}

/// The size of the group is read from the deployment, not from a property.
///
/// This is the whole point of the `peers` signal. A component cannot see its own
/// surroundings, so the engine states how many replicas of the member sit on the
/// far end of the relationship, and the majority follows from that. Restating it
/// here as a property is how a model comes to disagree with the scale unit
/// sitting beside it.
#[test]
fn the_group_counts_the_replicas_of_its_member() {
    for (replicas, nodes, quorum) in [("3", 3.0, 2.0), ("5", 5.0, 3.0), ("4", 4.0, 3.0)] {
        let solved = solve(&grouped(Some(replicas)));
        close(get(&solved, "group", "nodes"), nodes, "nodes in the group");
        close(get(&solved, "group", "quorum"), quorum, "replies awaited");
    }
}

/// An even group awaits as many replies as the odd group above it.
///
/// Four nodes need three, exactly as five do, so the fourth node costs a node to
/// run and a node's availability to lose while buying nothing. Worth having the
/// model say so, because "more replicas is safer" is the intuition it corrects.
#[test]
fn an_even_group_buys_nothing_over_the_odd_one_below_it() {
    let three = solve(&fallible(Some("3"), "0.02"));
    let four = solve(&fallible(Some("4"), "0.02"));
    close(get(&three, "group", "quorum"), 2.0, "two of three");
    close(get(&four, "group", "quorum"), 3.0, "three of four");
    assert!(
        get(&four, "group", "success_rate") < get(&three, "group", "success_rate"),
        "a fourth node awaiting a third reply must be less available, not more"
    );
}

/// A member with no scale unit around it is a group of one.
///
/// A design part-way through being drawn must still solve, and a single node is
/// a perfectly coherent thing for it to describe: one node, one reply awaited.
#[test]
fn an_unreplicated_member_is_a_group_of_one() {
    let solved = solve(&grouped(None));
    close(get(&solved, "group", "nodes"), 1.0, "one node");
    close(get(&solved, "group", "quorum"), 1.0, "one reply");
    close(
        get(&solved, "group", "success_rate"),
        get(&solved, "group", "node_success"),
        "a group of one is exactly its node",
    );
}

/// A scale unit enclosing both ends is deployed together, so it does not count.
///
/// Three copies of a cell each hold one quorum talking to one node, which is one
/// node — not three. Reading the member's replica count without dividing out the
/// units the caller is also inside would report every cell as a three-node group
/// and quietly triple the availability of the design.
#[test]
fn a_unit_enclosing_both_ends_is_not_a_group() {
    let solved = solve(
        "
components:
  - id: users
    name: Users
    type: client
    properties:
      request_rate: '300'
      latency_target: '1'
      success_target: '0.99'
  - id: group
    name: Group
    type: quorum
    properties:
      overhead: '0.001'
  - id: node
    name: Node
    type: compute
    properties:
      service_time: '0.02'
      parallelism: '400'
relationships:
  - from: users
    to: group
  - from: group
    to: node
scale_units:
  - id: cell
    name: Cell
    replicas: '3'
    distribution: sharded
    members: [group, node]
",
    );
    close(
        get(&solved, "group", "nodes"),
        1.0,
        "a cell's quorum talks to its own node, not to every cell's",
    );
}

/// A quorum's members all see every request, which is what it costs.
///
/// A quorum buys latency and availability, never throughput. Every node receives
/// every request and the group's ceiling is one node's, so a design reaching for
/// a quorum to go faster under load has reached for the wrong component.
#[test]
fn every_node_receives_every_request() {
    let solved = solve(&grouped(Some("3")));
    close(get(&solved, "group", "arriving"), 100.0, "arrivals");
    close(get(&solved, "group", "replicated"), 100.0, "sent to a node");
    close(get(&solved, "node", "arriving"), 100.0, "reaching a node");
    close(get(&solved, "group", "issued"), 300.0, "across the group");
    close(
        get(&solved, "group", "node_capacity"),
        get(&solved, "node", "capacity"),
        "the ceiling is one node's however many are added",
    );
}

/// Availability rises with the group, which no other catalogue type does.
///
/// The chance a majority holds is the binomial upper tail, and it is checked
/// against the law rather than a recorded figure so that the test says what it
/// believes: three nodes at ninety-nine percent reach four nines together.
#[test]
fn a_majority_is_more_available_than_any_of_its_nodes() {
    let solved = solve(&fallible(Some("3"), "0.02"));
    let node = get(&solved, "group", "node_success");
    let group = get(&solved, "group", "success_rate");
    close(
        group,
        number(&format!("Reliability.quorumSuccess({node}, 3, 2)")),
        "the binomial upper tail",
    );
    assert!(
        group > node,
        "a majority of three must beat any one of them, got {group} against {node}"
    );
    close(
        number("Reliability.quorumSuccess(0.99, 3, 2)"),
        3.0 * 0.99 * 0.99 * 0.01 + 0.99_f64.powi(3),
        "two of three, written out",
    );
}

/// Waiting for a majority is faster than waiting for one node, let alone all.
///
/// The quorum-th reply of `n` exponential response times has mean
/// `L * (H_n - H_{n-r})`, so two of three is five sixths of a single node's wait
/// where all three would be eleven sixths of it. That gap is the tail latency a
/// quorum buys and a fan-out cannot.
#[test]
fn waiting_for_a_majority_is_faster_than_waiting_for_a_node() {
    let solved = solve(&grouped(Some("3")));
    let node = get(&solved, "group", "node_wait");
    close(
        get(&solved, "group", "quorum_wait"),
        node * (1.0 / 3.0 + 1.0 / 2.0),
        "the second of three exponential replies",
    );
    close(
        get(&solved, "group", "latency"),
        get(&solved, "group", "quorum_wait") + 0.001,
        "a majority replying, then agreeing on them",
    );
    assert!(
        get(&solved, "group", "quorum_wait") < node,
        "a majority must arrive sooner than the average node does"
    );
    close(
        number("Reliability.quorumLatency(1, 3, 3)"),
        1.0 / 3.0 + 1.0 / 2.0 + 1.0,
        "waiting for every node is the harmonic sum in full",
    );
}

/// A group whose nodes are separate components is refused rather than averaged.
///
/// The figures a quorum reads are one node's. Success multiplies across
/// arrivals, so a second relationship would hand this component the chance that
/// *every* node succeeded where it wanted the chance that one did, and report a
/// healthy group as a failing one. Saying so is better than being quietly wrong.
#[test]
fn a_second_relationship_onto_the_member_port_is_refused() {
    let error = try_solve(
        "
components:
  - id: users
    name: Users
    type: client
    properties:
      request_rate: '100'
      latency_target: '1'
      success_target: '0.99'
  - id: group
    name: Group
    type: quorum
  - id: first
    name: First
    type: compute
    properties:
      service_time: '0.02'
      parallelism: '400'
  - id: second
    name: Second
    type: compute
    properties:
      service_time: '0.02'
      parallelism: '400'
relationships:
  - from: users
    to: group
  - from: group
    to: first
  - from: group
    to: second
",
    )
    .expect_err("a quorum reads one member, not several");
    assert!(
        error.contains("admits one relationship"),
        "expected the crowded port to be named, got {error}"
    );
}
