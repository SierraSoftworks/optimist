//! Boundary-condition assessment of the shipped component types and behaviours.
//!
//! Every catalogue entry claims a physical law in its manifest: Little's Law for
//! an occupancy, a truncated geometric count for a retry budget, an Erlang race
//! for a deadline. Those claims are only worth as much as they hold at the edges,
//! because the edges are where a capacity model is read. A design nobody worries
//! about is not modelled at all.
//!
//! Each test here builds the smallest system that can exercise one claim and
//! checks the solved figures against the closed form, at rest, at exactly the
//! limit, and beyond it. Where a quantity has no meaning at the boundary — an
//! idle queue's waiting time, an unattached branch's ceiling — the test states
//! what the model reports instead and why that reading is the useful one.

use std::collections::BTreeMap;

use optimist::squiggle::Value;
use optimist::system::{
    Bottleneck, EvaluationConfig, LinkState, SystemModel, bottlenecks, builtin_catalogue, evaluate,
};

/// Draws enough to resolve a blocking probability of a percent or so, and few
/// enough that a test file's worth of solves stays quick.
fn config() -> EvaluationConfig {
    EvaluationConfig {
        seed: 7,
        sample_count: 500,
        ..EvaluationConfig::default()
    }
}

/// A solved model, addressed the way the manifests are written.
#[derive(Debug)]
struct Solved {
    channels: BTreeMap<String, BTreeMap<String, Value>>,
    links: BTreeMap<String, LinkState>,
    ranked: Vec<Bottleneck>,
}

impl Solved {
    /// Mean of one component's channel.
    fn get(&self, component: &str, channel: &str) -> f64 {
        let channels = self
            .channels
            .get(component)
            .unwrap_or_else(|| panic!("no component '{component}'"));
        mean(
            channels
                .get(channel)
                .unwrap_or_else(|| panic!("no channel '{component}.{channel}'")),
        )
    }

    /// Mean of one quantity on the wire between two components.
    fn wire(&self, from: &str, to: &str, quantity: &str) -> f64 {
        let key = self
            .links
            .keys()
            .find(|id| id.starts_with(&format!("{from}.")) && id.contains(&format!(" to {to}.")))
            .unwrap_or_else(|| panic!("no wire from '{from}' to '{to}'"));
        let link = &self.links[key];
        match quantity {
            "backlog" => mean(&link.backlog),
            "wait" => mean(&link.wait),
            "blocked" => mean(&link.blocked),
            "offered" => mean(&link.offered),
            "drain" => mean(&link.drain),
            "transfer" => mean(&link.transfer),
            other => panic!("no wire quantity '{other}'"),
        }
    }

    /// Mean utilisation of one constraint.
    fn constraint(&self, component: &str, constraint: &str) -> f64 {
        self.ranked
            .iter()
            .find(|entry| entry.component.as_str() == component && entry.constraint == constraint)
            .unwrap_or_else(|| panic!("no constraint '{component}.{constraint}'"))
            .utilisation
    }
}

fn mean(value: &Value) -> f64 {
    match value {
        Value::Number(number) => *number,
        Value::Distribution(distribution) => distribution.mean().expect("mean"),
        other => panic!("expected a quantity, got {other:?}"),
    }
}

/// Solves a model written the way an author would write it.
fn solve(source: &str) -> Solved {
    match solved(source, config()) {
        Ok(solved) => solved,
        Err(error) => panic!("expected the model to solve: {error}"),
    }
}

/// Solves a model with the solver taking steps of a stated length.
fn solve_stepping(source: &str, step: f64) -> Solved {
    let config = EvaluationConfig { step, ..config() };
    match solved(source, config) {
        Ok(solved) => solved,
        Err(error) => panic!("expected the model to solve: {error}"),
    }
}

/// Solves a model, surfacing the diagnostic rather than the result.
fn try_solve(source: &str) -> Result<Solved, String> {
    solved(source, config())
}

fn solved(source: &str, config: EvaluationConfig) -> Result<Solved, String> {
    let model: SystemModel = serde_yaml_ng::from_str(source).expect("the model parses");
    let catalogue = builtin_catalogue().expect("catalogue");
    let evaluation = evaluate(&model, &catalogue, config)
        .map_err(|error: optimist::system::EvaluationError| error.to_string())?;
    assert!(evaluation.converged(), "the model did not settle");
    let step = evaluation.settled();
    let ranked = bottlenecks(&model, &catalogue, step, config)
        .map_err(|error: optimist::system::EvaluationError| error.to_string())?;
    Ok(Solved {
        channels: step
            .components
            .iter()
            .map(|(id, state)| (id.to_string(), state.channels.clone()))
            .collect(),
        links: step
            .links
            .iter()
            .map(|(id, state)| (id.to_string(), state.clone()))
            .collect(),
        ranked,
    })
}

/// Asserts two figures agree to within a relative tolerance.
#[track_caller]
fn close(actual: f64, expected: f64, what: &str) {
    let tolerance = expected.abs().max(1.0) * 1e-6;
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: expected {expected}, got {actual}"
    );
}

/// Asserts two figures agree to within a percentage point or so.
///
/// Used where a figure is a mean over draws of a blocking probability, which is
/// exact in the closed form and sampled here.
#[track_caller]
fn near(actual: f64, expected: f64, slack: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= slack,
        "{what}: expected {expected} within {slack}, got {actual}"
    );
}

const CLIENT: &str = "
  - id: users
    name: Users
    type: client
    properties:
      request_rate: '%RATE%'
";

/// A client offering `rate` into a pool with `parallelism` slots at 20 ms each.
fn pool(rate: &str, parallelism: &str) -> String {
    format!(
        "
components:
{}
  - id: api
    name: API
    type: compute
    properties:
      service_time: '0.02'
      parallelism: '{parallelism}'
relationships:
  - from: users
    to: api
",
        CLIENT.replace("%RATE%", rate)
    )
}

// ---------------------------------------------------------------------------
// compute
// ---------------------------------------------------------------------------

/// Capacity is Little's Law on the slots and the time each request holds one.
///
/// Both levers move throughput identically and neither moves latency, which is
/// the distinction the manifest exists to make: doubling the pool and halving
/// the service time buy the same rate, and only one of them makes a request
/// faster.
#[test]
fn a_pool_sustains_its_slots_over_its_hold_time() {
    let solved = solve(&pool("40", "8"));
    // Eight slots at twenty milliseconds each.
    close(solved.get("api", "capacity"), 400.0, "capacity");
    close(solved.get("api", "servers"), 8.0, "servers");
    close(solved.get("api", "hold_time"), 0.02, "hold time");
    close(solved.get("api", "utilisation"), 0.1, "utilisation");

    let doubled = solve(&pool("40", "16"));
    close(
        doubled.get("api", "capacity"),
        800.0,
        "capacity with twice the slots",
    );
    close(
        doubled.get("api", "residence"),
        solved.get("api", "residence"),
        "residence is unmoved by the slot count",
    );
}

/// At exactly one the buffer in front is half full and refuses one draw in K+1.
///
/// This is the boundary the whole model turns on. Below it a queue is invisible
/// and above it a design is plainly broken; at it, the M/M/1/K result says the
/// hundred-deep wire holds fifty and turns away one arrival in a hundred and
/// one. A model that reported nothing waiting at unit utilisation would hide
/// the only place a design gives warning before it fails.
#[test]
fn at_unit_utilisation_the_wire_is_half_full() {
    let solved = solve(&pool("400", "8"));
    close(solved.get("api", "utilisation"), 1.0, "utilisation");
    // The default wire holds a hundred operations.
    near(solved.wire("users", "api", "backlog"), 50.0, 0.5, "backlog");
    near(
        solved.wire("users", "api", "blocked"),
        1.0 / 101.0,
        1e-3,
        "blocking probability",
    );
    near(
        solved.get("users", "success"),
        1.0 - 1.0 / 101.0,
        1e-3,
        "success seen by the caller",
    );
    close(
        solved.constraint("api", "capacity"),
        1.0,
        "capacity constraint",
    );
}

/// Beyond saturation the pool serves its capacity and the wire refuses the rest.
///
/// Refusal is charged once, on the wire, and the share refused is the
/// reciprocal of the load. A pool asked for ten times what it can serve answers
/// a tenth, and says so through the caller's success rather than by quietly
/// serving less and reporting itself healthy.
#[test]
fn beyond_saturation_the_shortfall_is_charged_once() {
    let solved = solve(&pool("4000", "8"));
    close(solved.get("api", "utilisation"), 10.0, "utilisation");
    // What travels onward is bounded by what the pool can serve.
    close(solved.get("api", "calls"), 400.0, "calls");
    near(
        solved.wire("users", "api", "blocked"),
        0.9,
        1e-3,
        "blocked share",
    );
    near(solved.get("users", "success"), 0.1, 1e-3, "success");
    // The pool itself reports serving what reached it, not the overload.
    close(
        solved.get("api", "success_rate"),
        1.0,
        "the pool's own success",
    );
}

/// A pool with nothing to do is not a pool that is failing.
///
/// The success rate is a ratio of two quantities that are both zero at rest, and
/// reading it as nought would report every unexercised path as a total outage.
/// Because success multiplies back along the call graph, one such path is enough
/// to take a whole design's reported reliability to zero — which is exactly what
/// a feature flag set to nought is meant to be useful for.
#[test]
fn an_idle_pool_reports_no_failures() {
    let solved = solve(&pool("0", "8"));
    close(solved.get("api", "arriving"), 0.0, "arrivals");
    close(
        solved.get("api", "success_rate"),
        1.0,
        "success rate at rest",
    );
    close(
        solved.get("users", "success"),
        1.0,
        "success seen by the caller",
    );
    close(solved.get("users", "failure"), 0.0, "failures");
    assert!(
        solved.constraint("users", "success_objective") < 1.0,
        "an idle design must not report its objective as exhausted"
    );
}

/// A slow dependency consumes the caller's capacity without touching its demand.
///
/// This is the coupling the manifest is written around: a worker is held for the
/// whole of a call, so the dependency's latency lands in the caller's hold time
/// and divides its throughput. Nothing about the caller changed.
#[test]
fn a_dependency_s_latency_becomes_the_caller_s_capacity() {
    let solved = solve(
        "
components:
  - id: users
    name: Users
    type: client
    properties:
      request_rate: '10'
  - id: api
    name: API
    type: compute
    properties:
      service_time: '0.02'
      parallelism: '8'
  - id: store
    name: Store
    type: datastore
    properties:
      operation_limit: '10000'
      transfer_limit: '1e9'
      volume_limit: '1e12'
      record_size: '100'
      retention: '60'
      service_time: '0.08'
relationships:
  - from: users
    to: api
  - from: api
    to: store
",
    );
    close(
        solved.get("api", "dependency_wait"),
        0.08,
        "dependency wait",
    );
    close(solved.get("api", "hold_time"), 0.1, "hold time");
    // Eight slots at a tenth of a second: eighty a second, a fifth of what the
    // same pool sustained with no dependency at all.
    close(solved.get("api", "capacity"), 80.0, "capacity");
}

/// A pool with no slots is a modelling mistake rather than a pool of no capacity.
#[test]
fn a_pool_with_no_slots_is_rejected() {
    let error = try_solve(&pool("100", "0")).expect_err("a pool with no slots cannot be solved");
    assert!(
        error.contains("utilisation"),
        "expected the undefined channel to be named, got {error}"
    );
}

// ---------------------------------------------------------------------------
// datastore
// ---------------------------------------------------------------------------

fn store(rate: &str, concurrency_limit: &str, record_size: &str) -> String {
    format!(
        "
components:
{}
  - id: api
    name: API
    type: compute
    properties:
      service_time: '0.001'
      parallelism: '2000'
  - id: store
    name: Store
    type: datastore
    properties:
      operation_limit: '1000'
      transfer_limit: '1e6'
      volume_limit: '1e12'
      record_size: '{record_size}'
      retention: '3600'
      service_time: '0.005'
      concurrency_limit: '{concurrency_limit}'
relationships:
  - from: users
    to: api
  - from: api
    to: store
",
        CLIENT.replace("%RATE%", rate)
    )
}

/// Every derived quantity is the operation rate carried through one conversion.
///
/// Bytes per second is operations times record size, resident records are
/// operations times retention by Little's Law, and resident bytes are those
/// records times the same record size. Getting any one of these wrong is a
/// dimensional mistake rather than a modelling one, and it would not announce
/// itself in a figure that still looks like a number.
#[test]
fn a_store_converts_its_operation_rate_consistently() {
    let solved = solve(&store("100", "infinity", "1000"));
    close(solved.get("store", "operations"), 100.0, "operations");
    close(solved.get("store", "transfer"), 100_000.0, "transfer");
    close(solved.get("store", "records"), 360_000.0, "records");
    close(solved.get("store", "volume"), 360_000_000.0, "volume");
    close(
        solved.get("store", "volume"),
        solved.get("store", "records") * 1000.0,
        "volume against records",
    );
}

/// Which limit binds is decided by record size, not by the store.
///
/// The same operation rate against the same device saturates the transfer path
/// with large records and the operation path with small ones, which is why a
/// store is not describable by a single number.
#[test]
fn record_size_decides_which_limit_a_store_meets() {
    // A megabyte a second and a thousand operations a second.
    let small = solve(&store("900", "infinity", "100"));
    assert!(small.constraint("store", "operations") > small.constraint("store", "transfer"));
    let large = solve(&store("900", "infinity", "5000"));
    assert!(large.constraint("store", "transfer") > large.constraint("store", "operations"));
}

/// Latency is the service time stretched by the simultaneous capacity in use.
///
/// This is the term that makes a store and a caller holding connections into a
/// pair that can rest in more than one place: the delay decides how much is
/// held, and how much is held decides the delay.
#[test]
fn a_store_s_latency_stretches_with_its_concurrency() {
    // Nothing is held against an unlimited store, so it never stretches.
    let unlimited = solve(&store("100", "infinity", "1000"));
    close(
        unlimited.get("store", "concurrency"),
        0.0,
        "concurrency share",
    );
    close(unlimited.get("store", "latency"), 0.005, "latency at rest");

    let limited = solve(&store("100", "1000", "1000"));
    let share = limited.get("store", "concurrency");
    assert!(share > 0.0, "some of the store must be spoken for");
    close(
        limited.get("store", "latency"),
        0.005 / (1.0 - share),
        "latency against the stretch its own concurrency implies",
    );
}

/// Past its simultaneous limit the stretch saturates rather than diverging.
///
/// A thousandfold is a sentinel, not a prediction. It is unmistakable against
/// any real budget while staying a number, which is what a design past this
/// point needs: the reading is that the store is out of connections, not that a
/// request will take five seconds.
#[test]
fn a_store_past_its_concurrency_limit_saturates() {
    let solved = solve(&store("100", "1", "1000"));
    assert!(
        solved.get("store", "concurrency") >= 1.0,
        "the store must be past its simultaneous limit for this to be the boundary"
    );
    close(solved.get("store", "latency"), 5.0, "saturated latency");
    assert!(
        solved.constraint("store", "concurrency") > 1.0,
        "the constraint must report the exhaustion the latency is standing in for"
    );
    // And the rate limit is nowhere near binding, which is the point.
    assert!(
        solved.constraint("store", "operations") < 0.5,
        "a store well inside its rate limit can still be the thing that fails"
    );
}

/// A store nothing is asking for is not a store that is failing.
#[test]
fn an_idle_store_reports_no_failures() {
    let solved = solve(&store("0", "100", "1000"));
    close(solved.get("store", "success_rate"), 1.0, "success at rest");
    close(
        solved.get("api", "dependency_success"),
        1.0,
        "success seen by the caller",
    );
    close(
        solved.get("users", "success"),
        1.0,
        "success seen end to end",
    );
}

// ---------------------------------------------------------------------------
// queue
// ---------------------------------------------------------------------------

fn buffer(rate: &str, service_rate: &str, capacity: &str) -> String {
    format!(
        "
components:
{}
  - id: buffer
    name: Buffer
    type: queue
    properties:
      service_rate: '{service_rate}'
      capacity: '{capacity}'
relationships:
  - from: users
    to: buffer
",
        CLIENT.replace("%RATE%", rate)
    )
}

/// Below the drain rate a queue rests where the stationary result says it does.
///
/// The same M/M/1/K law the buffer on every relationship uses, so the two agree
/// rather than each reporting its own arithmetic. A queue driven at half its
/// drain rate holds one unit of work on average, which is small enough to be
/// invisible in a latency budget and is not nought.
#[test]
fn a_queue_below_its_drain_rate_rests_where_the_stationary_result_says() {
    let solved = solve(&buffer("50", "100", "500"));
    close(solved.get("buffer", "departures"), 50.0, "departures");
    close(solved.get("buffer", "load"), 0.5, "load");
    // Little's Law on the unbounded result, which a five-hundred-deep buffer at
    // half load is indistinguishable from.
    close(solved.get("buffer", "backlog"), 1.0, "backlog");
    close(
        solved.get("buffer", "wait"),
        solved.get("buffer", "backlog") / solved.get("buffer", "departures"),
        "waiting time against Little's Law",
    );
    close(
        solved.get("buffer", "accepted_ratio"),
        1.0,
        "accepted share",
    );
}

/// At exactly its drain rate a queue is half full, which is the warning.
///
/// No stationary result exists for an unbounded queue at unit load, and the
/// bounded one is uniform over its occupancies: every depth equally likely, so
/// the mean is half the buffer and one arrival in `K + 1` is refused. A design
/// reading nought here would be told it was fine at the exact point it stopped
/// being fine.
#[test]
fn at_its_drain_rate_a_queue_is_half_full() {
    let solved = solve(&buffer("100", "100", "500"));
    close(solved.get("buffer", "departures"), 100.0, "departures");
    close(solved.get("buffer", "backlog"), 250.0, "backlog");
    close(solved.get("buffer", "wait"), 2.5, "waiting time");
    near(
        solved.get("buffer", "accepted_ratio"),
        1.0 - 1.0 / 501.0,
        1e-9,
        "accepted share",
    );
    close(
        solved.constraint("buffer", "throughput"),
        1.0,
        "throughput at balance",
    );
    // And the same reading the wire in front of a component would give.
    close(
        solved.get("buffer", "backlog"),
        solve(&pool("400", "8")).wire("users", "api", "backlog") * 5.0,
        "against the buffer on a relationship at the same load",
    );
}

/// Overload becomes backlog and waiting time, and Little's Law relates the two.
///
/// A queue cannot relieve a shortfall in the consumer's capacity. It changes who
/// waits, not how much work there is, and the waiting it produces is the resident
/// backlog over the rate draining it.
#[test]
fn an_overloaded_queue_turns_the_excess_into_waiting() {
    let solved = solve(&buffer("300", "100", "500"));
    close(
        solved.get("buffer", "departures"),
        100.0,
        "departures are capped at the drain",
    );
    // Sustained overload fills the buffer; the depth is all that bounds it.
    assert!(
        solved.get("buffer", "backlog") > 490.0,
        "expected the buffer to be all but full, got {}",
        solved.get("buffer", "backlog")
    );
    close(
        solved.get("buffer", "wait"),
        solved.get("buffer", "backlog") / solved.get("buffer", "departures"),
        "waiting time against Little's Law",
    );
    close(solved.constraint("buffer", "throughput"), 3.0, "throughput");
}

/// Where a design rests is not a fact about the solver's step.
///
/// The queue is the one shipped type that carries state between steps, and
/// integrating from empty would make its resting backlog whatever one step
/// happened to accumulate. Two designs solved at different steps would then be
/// compared on the solver rather than on themselves.
#[test]
fn a_resting_queue_does_not_depend_on_the_solver_s_step() {
    for rate in ["50", "100", "300", "1000"] {
        let source = buffer(rate, "100", "500");
        let coarse = solve_stepping(&source, 1.0);
        let fine = solve_stepping(&source, 0.01);
        for channel in ["backlog", "wait", "accepted_ratio", "departures"] {
            close(
                fine.get("buffer", channel),
                coarse.get("buffer", channel),
                &format!("{channel} at {rate} offered"),
            );
        }
    }
}

/// A buffer never holds more than its depth, and refuses what it cannot store.
///
/// The two have to agree. A queue reporting that it turned work away for want of
/// room, and in the same breath that it stored every unit it turned away, would
/// charge callers for waiting behind a backlog that could never have formed.
#[test]
fn a_full_queue_refuses_rather_than_overfilling() {
    let solved = solve(&buffer("1000", "100", "50"));
    assert!(
        solved.get("buffer", "backlog") <= 50.0,
        "a buffer cannot hold more than its depth, got {}",
        solved.get("buffer", "backlog")
    );
    // Ten times the drain rate, so a tenth of it gets in and the rest is refused.
    near(
        solved.get("buffer", "accepted_ratio"),
        0.1,
        1e-6,
        "accepted share",
    );
    close(
        solved.get("buffer", "wait"),
        solved.get("buffer", "backlog") / 100.0,
        "waiting time is bounded by the depth",
    );
    assert!(
        solved.constraint("buffer", "depth") <= 1.0,
        "a buffer cannot be more than full"
    );
}

/// An idle queue has no waiting time to report rather than an infinite one.
///
/// Little's Law divides by the rate draining the buffer, and with nothing
/// resident there is no unit of work whose wait could be asked about.
#[test]
fn an_idle_queue_reports_no_wait() {
    let solved = solve(&buffer("0", "100", "500"));
    close(solved.get("buffer", "wait"), 0.0, "waiting time");
    close(solved.get("buffer", "backlog"), 0.0, "backlog");
    close(
        solved.get("buffer", "accepted_ratio"),
        1.0,
        "accepted share",
    );
}

/// The producer is answered when the work is accepted, not when it is done.
///
/// That is the whole point of a queue, and it is why the delay travels onward to
/// the consumer as staleness rather than back to the producer as latency.
#[test]
fn a_queue_does_not_make_its_producer_wait() {
    let solved = solve(&buffer("300", "100", "500"));
    close(solved.get("users", "latency"), 0.0, "producer latency");
    assert!(
        solved.get("buffer", "wait") > 1.0,
        "the waiting has to exist somewhere for this to be the interesting case"
    );
}

// ---------------------------------------------------------------------------
// load balancer
// ---------------------------------------------------------------------------

fn balanced(rate: &str, admission_limit: &str, replicas: &str, service_time: &str) -> String {
    format!(
        "
components:
{}
  - id: edge
    name: Edge
    type: load-balancer
    properties:
      admission_limit: '{admission_limit}'
      connection_limit: '1000'
      replicas: '{replicas}'
      overhead: '0.001'
  - id: api
    name: API
    type: compute
    properties:
      service_time: '{service_time}'
      parallelism: '400'
relationships:
  - from: users
    to: edge
  - from: edge
    to: api
",
        CLIENT.replace("%RATE%", rate)
    )
}

/// Below the limit the balancer is a hop, and admitted demand splits by replica.
#[test]
fn a_balancer_below_its_limit_only_spreads_demand() {
    let solved = solve(&balanced("100", "500", "4", "0.02"));
    close(solved.get("edge", "admitted"), 100.0, "admitted");
    close(solved.get("edge", "shed"), 0.0, "shed");
    close(solved.get("edge", "per_replica"), 25.0, "per replica");
    close(solved.get("edge", "success_rate"), 1.0, "success");
}

/// Refusal at the door is charged once, and it is charged.
///
/// A design admitting a quarter of its traffic serves a quarter of it. Counting
/// the refusal both in the balancer's own success rate and again on the wire it
/// drains would report a sixteenth, which is not a conservative reading but a
/// wrong one: it contradicts the rate the backends are visibly answering.
#[test]
fn shedding_is_charged_once_and_charged_fully() {
    let solved = solve(&balanced("2000", "500", "1", "0.02"));
    close(solved.get("edge", "admitted"), 500.0, "admitted");
    close(solved.get("edge", "shed"), 1500.0, "shed");
    near(
        solved.get("users", "success"),
        0.25,
        1e-3,
        "success end to end",
    );
    // What the backends answer and what the caller believes succeeded agree.
    near(
        solved.get("api", "calls"),
        solved.get("users", "offered") * solved.get("users", "success"),
        1.0,
        "served rate against the caller's reading",
    );
    close(solved.constraint("edge", "admission"), 4.0, "admission");
}

/// Connections are Little's Law, so a slow backend exhausts them at fixed demand.
///
/// This is how a latency problem downstream becomes an availability problem at
/// the edge without anybody asking for more.
#[test]
fn backends_slowing_down_consume_the_connection_limit() {
    let quick = solve(&balanced("400", "5000", "1", "0.02"));
    close(
        quick.get("edge", "connections"),
        quick.get("edge", "admitted") * quick.get("edge", "latency"),
        "connections against Little's Law",
    );

    let slow = solve(&balanced("400", "5000", "1", "0.2"));
    close(
        slow.get("edge", "admitted"),
        quick.get("edge", "admitted"),
        "demand is unchanged",
    );
    assert!(
        slow.get("edge", "connections") > quick.get("edge", "connections") * 5.0,
        "ten times the backend latency must consume far more connections"
    );
}

/// A balancer with no backends is a modelling mistake, not a balancer of none.
#[test]
fn a_balancer_with_no_replicas_is_rejected() {
    let error = try_solve(&balanced("100", "500", "0", "0.02"))
        .expect_err("demand cannot be spread across no replicas");
    assert!(
        error.contains("per_replica"),
        "expected the undefined channel to be named, got {error}"
    );
}

// ---------------------------------------------------------------------------
// aggregator
// ---------------------------------------------------------------------------

fn fanned(branches: &str, attached: bool) -> String {
    let leaf = if attached {
        "
  - id: leaf
    name: Leaf
    type: compute
    properties:
      service_time: '0.01'
      parallelism: '30'
  - from: gather
    to: leaf
"
    } else {
        ""
    };
    let (component, relationship) = leaf.split_at(leaf.find("  - from:").unwrap_or(leaf.len()));
    format!(
        "
components:
{}
  - id: gather
    name: Gather
    type: aggregator
    properties:
      branches: '{branches}'
      overhead: '0.002'
{component}
relationships:
  - from: users
    to: gather
{relationship}
",
        CLIENT.replace("%RATE%", "100")
    )
}

/// One request becomes as many as there are branches.
///
/// This is the amplification a diagram of the component does not show, and the
/// commonest reason a dependency is busier than the design says it should be.
#[test]
fn a_fan_out_multiplies_the_load_behind_it() {
    let solved = solve(&fanned("3", true));
    close(solved.get("gather", "arriving"), 100.0, "arrivals");
    close(solved.get("gather", "fanned_out"), 300.0, "branch requests");
    close(
        solved.get("leaf", "arriving"),
        300.0,
        "load reaching the branch",
    );
    close(solved.constraint("gather", "fan_out"), 3.0, "amplification");
}

/// A branch's capacity reaches the caller divided by the calls made of it.
///
/// Fanning out consumes a branch several times over, so the rate this component
/// can accept is always smaller than the rate its branch can serve.
#[test]
fn a_fan_out_divides_the_capacity_it_reports() {
    let solved = solve(&fanned("3", true));
    // Thirty slots at ten milliseconds: three thousand a second, shared three
    // ways by every request that arrives.
    close(solved.get("leaf", "capacity"), 3_000.0, "branch capacity");
    close(
        solved.get("gather", "branch_capacity"),
        1_000.0,
        "capacity reported to callers",
    );
}

/// Waiting for the slowest branch and needing all of them both work against the
/// caller, and adding a branch is never free even when the branch is fast.
#[test]
fn a_fan_out_waits_for_the_slowest_and_needs_all_of_them() {
    let solved = solve(&fanned("3", true));
    close(
        solved.get("gather", "latency"),
        solved.get("gather", "branch_wait") + 0.002,
        "latency is the slowest branch plus combining",
    );
    close(
        solved.get("gather", "success_rate"),
        solved.get("gather", "branch_success"),
        "success is what every branch managed together",
    );
}

/// A fan-out with nothing attached still solves, and imposes no ceiling.
///
/// The smallest of no limits is no limit at all, and a design part-way through
/// being drawn must not fail to evaluate because a branch has not been wired up
/// yet. The figure reported is a saturation sentinel: far above any rate a real
/// service reaches, and therefore read as no ceiling by everything upstream.
#[test]
fn an_unattached_fan_out_imposes_no_ceiling() {
    let solved = solve(&fanned("3", false));
    close(solved.get("gather", "arriving"), 100.0, "arrivals");
    close(solved.get("gather", "fanned_out"), 300.0, "branch requests");
    assert!(
        solved.get("gather", "branch_capacity") > 1e12,
        "an unattached branch must not appear to limit its caller"
    );
    close(solved.get("users", "success"), 1.0, "success end to end");
}

// ---------------------------------------------------------------------------
// client
// ---------------------------------------------------------------------------

/// The client is the measurement point, and its objectives are the whole design's.
///
/// Latency and success arrive here with every hop and every behaviour along the
/// way already folded in, which is what turns "does this design meet its
/// objective" into a constraint rather than a figure assembled by hand.
#[test]
fn a_client_reads_the_whole_design_end_to_end() {
    let solved = solve(
        "
components:
  - id: users
    name: Users
    type: client
    properties:
      request_rate: '400'
      latency_target: '0.1'
      success_target: '0.99'
  - id: edge
    name: Edge
    type: load-balancer
    properties:
      admission_limit: '5000'
      connection_limit: '10000'
      overhead: '0.005'
  - id: api
    name: API
    type: compute
    properties:
      service_time: '0.02'
      parallelism: '200'
relationships:
  - from: users
    to: edge
  - from: edge
    to: api
",
    );
    // Every hop's own contribution, accumulated, plus the wire in front of it.
    near(
        solved.get("users", "latency"),
        solved.get("edge", "latency") + solved.wire("users", "edge", "wait"),
        1e-6,
        "latency at the client",
    );
    assert!(
        solved.get("users", "latency") > 0.025,
        "the client must see the edge's overhead and the pool's service time"
    );
    close(
        solved.get("users", "failure"),
        400.0 * (1.0 - solved.get("users", "success")),
        "failures against offered demand and success",
    );
    close(
        solved.constraint("users", "latency_objective"),
        solved.get("users", "latency") / 0.1,
        "the latency objective is observed against target",
    );
}

/// Objectives nobody set do not bind.
///
/// An unset deadline is unbounded and an unset success target admits anything,
/// so a design that has not said what it is for reports on capacity alone rather
/// than failing an objective it never had.
#[test]
fn unset_objectives_never_bind() {
    let solved = solve(&pool("40", "8"));
    close(
        solved.constraint("users", "latency_objective"),
        0.0,
        "latency objective",
    );
    assert!(
        solved.constraint("users", "success_objective") < 1e-3,
        "an unset success target must leave room for anything"
    );
}

// ---------------------------------------------------------------------------
// behaviours
// ---------------------------------------------------------------------------

/// A pool with `parallelism` slots reached through `mutators`.
fn behaved(rate: &str, parallelism: &str, mutators: &str) -> String {
    format!(
        "
components:
{}
  - id: api
    name: API
    type: compute
    properties:
      service_time: '0.02'
      parallelism: '{parallelism}'
relationships:
  - from: users
    to: api
{mutators}
",
        CLIENT.replace("%RATE%", rate)
    )
}

fn attached(id: &str, properties: &[(&str, &str)]) -> String {
    let mut source = format!("    mutators:\n      - type: {id}\n");
    if !properties.is_empty() {
        source.push_str("        properties:\n");
        for (name, value) in properties {
            source.push_str(&format!("          {name}: '{value}'\n"));
        }
    }
    source
}

/// Every behaviour has a setting at which it does nothing, and says so.
///
/// A policy that could not be disabled without deleting it would make an
/// intervention that turns one off impossible to express, and each of these is
/// the identity a rebinding has to be able to reach.
#[test]
fn each_behaviour_has_a_setting_at_which_it_does_nothing() {
    let plain = solve(&behaved("100", "8", ""));
    let identities = [
        attached("retry", &[("attempts", "1")]),
        attached("timeout", &[("budget", "1e9")]),
        attached("cache", &[("hit_ratio", "0")]),
        attached("fan-out", &[("branches", "1")]),
        attached("batch", &[("size", "1"), ("max_delay", "0")]),
        attached("load-shed", &[("limit", "1e9")]),
        attached("feature-flag", &[("exposure", "1")]),
        attached(
            "fallible",
            &[("transmit_failure", "0"), ("receive_failure", "0")],
        ),
    ];
    for identity in identities {
        let solved = solve(&behaved("100", "8", &identity));
        close(
            solved.get("api", "offered"),
            plain.get("api", "offered"),
            &format!("demand under {identity}"),
        );
        close(
            solved.get("users", "latency"),
            plain.get("users", "latency"),
            &format!("latency under {identity}"),
        );
        close(
            solved.get("users", "success"),
            plain.get("users", "success"),
            &format!("success under {identity}"),
        );
    }
}

/// A retry policy multiplies demand by the attempts it expects to make.
///
/// The count is a truncated geometric one, read from the success rate coming
/// back rather than supplied as a constant, so the amplification grows exactly
/// when the dependency starts failing. That is the loop that turns a transient
/// fault into a storm, and it is invisible unless it is modelled against the
/// failures actually observed.
#[test]
fn retrying_amplifies_demand_by_the_attempts_it_expects() {
    let solved = solve(&behaved(
        "800",
        "8",
        &attached("retry", &[("attempts", "3")]),
    ));
    // What the wire in front of the pool got through, per attempt.
    let attempt = 1.0 - solved.wire("users", "api", "blocked");
    let expected = (1.0 - (1.0 - attempt).powi(3)) / attempt;
    close(
        solved.wire("users", "api", "offered"),
        800.0 * expected,
        "amplified demand against the truncated geometric count",
    );
    near(
        solved.get("users", "success"),
        1.0 - (1.0 - attempt).powi(3),
        1e-3,
        "success against three independent attempts",
    );
    assert!(
        solved.constraint("api", "capacity") > 4.0,
        "a policy answering failure with load must make the shortfall worse"
    );
}

/// Retrying a healthy dependency costs nothing, which is why the policy survives.
#[test]
fn retrying_a_healthy_dependency_is_free() {
    let plain = solve(&behaved("100", "8", ""));
    let retried = solve(&behaved(
        "100",
        "8",
        &attached("retry", &[("attempts", "5")]),
    ));
    close(
        retried.get("api", "offered"),
        plain.get("api", "offered"),
        "demand against a dependency that is not failing",
    );
}

/// A deadline bounds what a caller waits and converts the tail into failure.
///
/// The race is against an exponential service time, so a budget equal to the
/// observed latency is met in a shade under two draws in three. Setting a
/// deadline at the mean is not setting it generously.
#[test]
fn a_deadline_bounds_latency_and_turns_the_tail_into_failure() {
    let generous = solve(&behaved(
        "100",
        "8",
        &attached("timeout", &[("budget", "1e9")]),
    ));
    let observed = generous.get("users", "latency");

    let tight = solve(&behaved(
        "100",
        "8",
        &attached("timeout", &[("budget", "0.01")]),
    ));
    assert!(
        tight.get("users", "latency") <= 0.01 + 1e-9,
        "the caller cannot wait longer than the budget, got {}",
        tight.get("users", "latency")
    );
    assert!(
        tight.get("users", "success") < generous.get("users", "success"),
        "a deadline the dependency misses has to cost something"
    );
    assert!(
        observed > 0.01,
        "the budget has to be inside the observed latency for this to be the boundary"
    );
}

/// A deadline is charged once, where it was missed.
///
/// The Erlang race against an exponential service time gives `1 - exp(-D / S)`,
/// so a budget equal to the observed latency is met in a shade under two draws
/// in three. That figure has to arrive at the caller intact.
///
/// It is easy for it not to. The deadline withdraws the request, the withdrawal
/// travels forward as cancellation, and a component that also counted cancelled
/// requests as failures would apply the same probability a second time — and
/// once more for every further hop the cancellation reached. Squaring a
/// perfectly ordinary deadline turns two thirds into four ninths, which is a
/// design decision made by an accounting mistake.
#[test]
fn a_deadline_is_charged_once_rather_than_at_every_hop() {
    let solved = solve(&behaved(
        "100",
        "8",
        &attached("timeout", &[("budget", "0.02")]),
    ));
    // The pool is comfortably inside its capacity, so nothing is refused and the
    // deadline is the only thing failing anybody.
    assert!(
        solved.constraint("api", "capacity") < 1.0,
        "the pool must not be saturated for the deadline to be the only cause"
    );
    let latency = solved.get("api", "residence") + solved.wire("users", "api", "wait");
    let met = 1.0 - (-0.02 / latency).exp();
    near(
        solved.get("users", "success"),
        met,
        1e-3,
        "success against the share that met the deadline",
    );
    // And emphatically not the square of it.
    assert!(
        solved.get("users", "success") > met * met * 1.2,
        "the deadline must not be charged twice, got {} against {}",
        solved.get("users", "success"),
        met * met
    );
    // The pool reports on what it did, not on what its caller gave up waiting for.
    close(
        solved.get("api", "success_rate"),
        1.0,
        "the pool's own success",
    );
}

/// Cancelling relieves the dependency by less than it fails the caller.
///
/// Half of any cancellation lands with the work already underway, so the
/// resource is spent and only the reply is thrown away. A deadline set tightly
/// against a struggling dependency therefore buys back half of what it appears
/// to.
#[test]
fn cancelling_saves_half_of_what_it_withdraws() {
    let solved = solve(&behaved(
        "800",
        "8",
        &attached("timeout", &[("budget", "0.05")]),
    ));
    let cancelled = solved.get("api", "cancelled");
    assert!(
        cancelled > 0.0,
        "the deadline has to bind for this to be the boundary"
    );
    close(solved.get("api", "salvaged"), cancelled / 2.0, "salvaged");
    close(
        solved.get("api", "offered"),
        solved.get("api", "arriving") - cancelled / 2.0,
        "load relieved against load withdrawn",
    );
}

/// A hop that drops cancellation strands the work behind it.
///
/// The caller gives up either way; what changes is whether the dependency finds
/// out. Remove the withdrawal and the load stays while the useful work falls,
/// which is the shape of a system that stays down after its cause has passed.
#[test]
fn dropping_cancellation_leaves_the_load_behind() {
    let honoured = solve(&behaved(
        "800",
        "8",
        &attached("timeout", &[("budget", "0.05")]),
    ));
    let stranded = solve(&behaved(
        "800",
        "8",
        "    mutators:\n      - type: timeout\n        properties:\n          budget: '0.05'\n      - type: ignores-cancellation\n",
    ));
    assert!(
        honoured.get("api", "cancelled") > 0.0,
        "the deadline has to bind for this to be the boundary"
    );
    close(
        stranded.get("api", "cancelled"),
        0.0,
        "cancellation past the hop",
    );

    // Work nobody withdrew is work the dependency is still doing, and the pool
    // is asked for the whole of the offered demand rather than what is still
    // wanted.
    close(
        stranded.get("api", "offered"),
        800.0,
        "load with nothing withdrawn",
    );
    assert!(
        honoured.get("api", "offered") < stranded.get("api", "offered"),
        "honouring cancellation has to relieve the dependency, got {} against {}",
        honoured.get("api", "offered"),
        stranded.get("api", "offered")
    );
    assert!(
        stranded.constraint("api", "capacity") > honoured.constraint("api", "capacity"),
        "and the relief has to show up as headroom the design would otherwise not have"
    );
}

/// How much a cancellation saves is a fact about the hop, not a constant.
///
/// Half by default, on the assumption that a cancellation is equally likely to
/// land at any point during a request. A hop that checks before starting work
/// saves nearly all of it and one that checks only before replying saves none,
/// and the difference is worth being able to state.
#[test]
fn how_much_a_cancellation_saves_can_be_stated_per_hop() {
    let solved = |effectiveness: Option<&str>| {
        let mut mutators =
            "    mutators:\n      - type: timeout\n        properties:\n          budget: '0.05'\n"
                .to_owned();
        if let Some(effectiveness) = effectiveness {
            mutators.push_str(&format!(
                "      - type: cancellation-effectiveness\n        properties:\n          effectiveness: '{effectiveness}'\n"
            ));
        }
        solve(&behaved("800", "8", &mutators))
    };

    // Unstated, and stated as the value it would have assumed, agree.
    let assumed = solved(None);
    let stated = solved(Some("0.5"));
    close(
        stated.get("api", "offered"),
        assumed.get("api", "offered"),
        "load with the assumption written down",
    );
    close(
        assumed.get("api", "salvage_share"),
        0.5,
        "the share assumed",
    );

    let cancelled = assumed.get("api", "cancelled");
    assert!(
        cancelled > 0.0,
        "the deadline has to bind for this to be the boundary"
    );

    // Nothing saved is the same load as never cancelling, and everything saved
    // is the most a deadline can buy back.
    let none = solved(Some("0"));
    close(none.get("api", "salvaged"), 0.0, "nothing salvaged");
    close(none.get("api", "offered"), 800.0, "load with nothing saved");
    let all = solved(Some("1"));
    close(
        all.get("api", "offered"),
        all.get("api", "arriving") - all.get("api", "cancelled"),
        "load with everything saved",
    );
    assert!(
        all.get("api", "offered") < assumed.get("api", "offered"),
        "saving all of a cancellation has to relieve more than saving half of it"
    );
}

/// A relationship carries bytes, and can be the thing that runs out.
///
/// Operation rates are what a capacity model usually reports on, and a design
/// whose rates all fit comfortably can still be bound by the bytes those
/// operations move. Sizes belong on the connection rather than on either end of
/// it, because the same store answers a key lookup and a document scan.
#[test]
fn a_link_can_be_the_bottleneck_rather_than_either_end_of_it() {
    let wire = |bandwidth: &str, request_size: &str, response_size: &str| {
        format!(
            "
components:
  - id: users
    name: Users
    type: client
    properties:
      request_rate: '1000'
  - id: api
    name: API
    type: compute
    properties:
      service_time: '0.001'
      parallelism: '200'
relationships:
  - from: users
    to: api
    bandwidth: '{bandwidth}'
    mutators:
      - type: message-size
        properties:
          request_size: '{request_size}'
          response_size: '{response_size}'
"
        )
    };
    // A thousand operations a second, a kilobyte out and nine back.
    let comfortable = solve(&wire("100e6", "1000", "9000"));
    close(
        comfortable.wire("users", "api", "transfer"),
        10_000_000.0,
        "bytes crossing the wire",
    );
    close(
        comfortable.constraint("users", "bandwidth"),
        0.1,
        "link utilisation",
    );
    // The pool is nowhere near its own limit, which is the point.
    assert!(comfortable.constraint("api", "capacity") < 0.01);

    let saturated = solve(&wire("8e6", "1000", "9000"));
    assert!(
        saturated.constraint("users", "bandwidth") > 1.0,
        "the link has to bind for this to be the boundary"
    );
    assert!(
        saturated.constraint("api", "capacity") < 0.01,
        "and it has to bind while both ends are idle"
    );
}

/// A link nobody gave a speed to is not a link that is full.
#[test]
fn an_unstated_link_speed_reports_no_constraint() {
    let solved = solve(&behaved(
        "1000",
        "200",
        &attached(
            "message-size",
            &[("request_size", "10000"), ("response_size", "10000")],
        ),
    ));
    assert!(
        solved
            .ranked
            .iter()
            .all(|entry| entry.constraint != "bandwidth"),
        "an unlimited link must not be reported as a constraint"
    );
    close(
        solved.wire("users", "api", "transfer"),
        20_000_000.0,
        "bytes crossing the wire",
    );
}

/// Batching leaves the bytes where they were while dividing the operations.
///
/// This is the whole of the trade, and the reason it is the right one against a
/// store limited by operations per second and the wrong one against a link.
#[test]
fn batching_leaves_the_byte_rate_unchanged() {
    let wire = |batch: &str| {
        format!(
            "
components:
  - id: users
    name: Users
    type: client
    properties:
      request_rate: '1000'
  - id: api
    name: API
    type: compute
    properties:
      service_time: '0.001'
      parallelism: '400'
relationships:
  - from: users
    to: api
    bandwidth: '1e9'
    mutators:
      - type: message-size
        properties:
          request_size: '500'
          response_size: '0'
{batch}
"
        )
    };
    let plain = solve(&wire(""));
    let batched = solve(&wire(
        "      - type: batch\n        properties:\n          size: '20'\n          max_delay: '0.01'\n",
    ));
    close(
        batched.get("api", "arriving"),
        50.0,
        "operations after batching",
    );
    close(
        batched.wire("users", "api", "transfer"),
        plain.wire("users", "api", "transfer"),
        "bytes after batching",
    );
    close(
        batched.constraint("users", "bandwidth"),
        plain.constraint("users", "bandwidth"),
        "link utilisation after batching",
    );
}

/// A cache is a capacity multiplier on the dependency, and only the misses travel.
#[test]
fn a_cache_passes_on_only_its_misses() {
    let solved = solve(&behaved(
        "1000",
        "8",
        &attached("cache", &[("hit_ratio", "0.9")]),
    ));
    close(
        solved.get("api", "offered"),
        100.0,
        "demand reaching the dependency",
    );
    close(
        solved.get("users", "offered"),
        1000.0,
        "demand the caller still makes",
    );

    let complete = solve(&behaved(
        "1000",
        "8",
        &attached("cache", &[("hit_ratio", "1")]),
    ));
    close(
        complete.get("api", "offered"),
        0.0,
        "a dependency nothing misses to",
    );
}

/// A hit ratio outside zero and one is clamped rather than believed.
///
/// A share of calls has no meaning above all of them, and an unclamped ratio
/// would send negative demand downstream — which every quantity derived from it
/// would then carry without complaint.
#[test]
fn a_cache_clamps_a_hit_ratio_it_cannot_honour() {
    let over = solve(&behaved(
        "1000",
        "8",
        &attached("cache", &[("hit_ratio", "1.5")]),
    ));
    close(
        over.get("api", "arriving"),
        0.0,
        "demand cannot be negative",
    );
    let under = solve(&behaved(
        "1000",
        "8",
        &attached("cache", &[("hit_ratio", "-0.5")]),
    ));
    close(
        under.get("api", "arriving"),
        1000.0,
        "a negative ratio caches nothing",
    );
}

/// Fan-out multiplies rate and occupancy alike.
///
/// Leaving the occupancy out is the same error as leaving the rate out, made
/// against whichever limit happens to bind, and it hides the amplification
/// entirely from a dependency sized by concurrency rather than throughput.
#[test]
fn fanning_out_multiplies_rate_and_occupancy_together() {
    let solved = solve(
        "
components:
  - id: users
    name: Users
    type: client
    properties:
      request_rate: '100'
  - id: api
    name: API
    type: compute
    properties:
      service_time: '0.001'
      parallelism: '400'
  - id: store
    name: Store
    type: datastore
    properties:
      operation_limit: '100000'
      transfer_limit: '1e9'
      volume_limit: '1e12'
      record_size: '100'
      retention: '60'
      service_time: '0.01'
      concurrency_limit: '1000'
relationships:
  - from: users
    to: api
  - from: api
    to: store
    mutators:
      - type: fan-out
        properties:
          branches: '6'
",
    );
    close(
        solved.get("store", "arriving"),
        600.0,
        "rate reaching the store",
    );
    close(
        solved.get("store", "held"),
        solved.get("api", "held_downstream") * 6.0,
        "occupancy reaching the store",
    );
}

/// Batching trades operation rate for waiting time.
///
/// The right trade against a store limited by operations per second, and the
/// wrong one against a caller with a deadline, because the delay is charged in
/// full to every call whose batch it waited for.
#[test]
fn batching_trades_operations_for_waiting() {
    let solved = solve(&behaved(
        "1000",
        "400",
        &attached("batch", &[("size", "10"), ("max_delay", "0.05")]),
    ));
    close(
        solved.get("api", "offered"),
        100.0,
        "operations after batching",
    );
    close(
        solved.get("users", "latency"),
        solved.get("api", "residence") + 0.05,
        "latency against the batch window",
    );
}

/// Shedding caps demand and is charged for, which is what makes it a choice.
///
/// Without the charge the model would report a shedding system as almost
/// perfectly successful while it refused most of its traffic, which is precisely
/// the reading that makes shedding look free.
#[test]
fn shedding_caps_demand_and_is_charged_for() {
    let solved = solve(&behaved(
        "400",
        "8",
        &attached("load-shed", &[("limit", "100")]),
    ));
    close(
        solved.get("api", "offered"),
        100.0,
        "demand after the limit",
    );
    near(
        solved.get("users", "success"),
        0.25,
        1e-3,
        "success against the share refused",
    );
    assert!(
        solved.constraint("api", "capacity") < 1.0,
        "shedding has to leave the dependency inside its limit"
    );
}

/// A flag admits a share of traffic and clamps a share it cannot honour.
#[test]
fn a_flag_admits_the_share_it_is_set_to() {
    let quarter = solve(&behaved(
        "400",
        "8",
        &attached("feature-flag", &[("exposure", "0.25")]),
    ));
    close(
        quarter.get("api", "offered"),
        100.0,
        "demand at a quarter exposure",
    );

    let off = solve(&behaved(
        "400",
        "8",
        &attached("feature-flag", &[("exposure", "0")]),
    ));
    close(
        off.get("api", "offered"),
        0.0,
        "a flag turned off starves what is behind it",
    );

    for exposure in ["2", "-1"] {
        let clamped = solve(&behaved(
            "400",
            "8",
            &attached("feature-flag", &[("exposure", exposure)]),
        ));
        let admitted = clamped.get("api", "offered");
        assert!(
            (0.0..=400.0).contains(&admitted),
            "an exposure of {exposure} must clamp rather than amplify, got {admitted}"
        );
    }
}

/// Request loss relieves the dependency; reply loss does not.
#[test]
fn a_fallible_link_loses_requests_and_replies_independently() {
    let solved = solve(&behaved(
        "100",
        "8",
        &attached(
            "fallible",
            &[("transmit_failure", "0.25"), ("receive_failure", "0.2")],
        ),
    ));
    close(
        solved.get("api", "offered"),
        75.0,
        "demand surviving transmit loss",
    );
    close(
        solved.get("users", "success"),
        0.6,
        "success surviving transmit and receive loss",
    );
}

/// Behaviours compose in the order they are declared, and the order matters.
///
/// Shedding before retrying caps what the policy may amplify; retrying before
/// shedding lets the amplified demand meet the cap instead.
#[test]
fn the_order_behaviours_are_declared_in_changes_the_answer() {
    let shed_first = solve(&behaved(
        "800",
        "8",
        "    mutators:\n      - type: load-shed\n        properties:\n          limit: '300'\n      - type: retry\n        properties:\n          attempts: '3'\n",
    ));
    let retry_first = solve(&behaved(
        "800",
        "8",
        "    mutators:\n      - type: retry\n        properties:\n          attempts: '3'\n      - type: load-shed\n        properties:\n          limit: '300'\n",
    ));
    assert!(
        shed_first.get("api", "offered") > retry_first.get("api", "offered"),
        "amplifying after a cap must place more load than capping after amplifying, got {} then {}",
        shed_first.get("api", "offered"),
        retry_first.get("api", "offered")
    );
    close(
        retry_first.get("api", "offered"),
        300.0,
        "a cap applied last is a cap",
    );
}
