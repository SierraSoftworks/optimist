//! The vocabulary of quantities that travel along a relationship.
//!
//! # Why signals are declared
//!
//! Components publish named quantities and consume them again downstream, and
//! the engine has to combine several arrivals into one figure without knowing
//! what any of them mean. It can only do that if each name says how it behaves.
//!
//! Two properties matter. The first is how arrivals combine: request rates from
//! several callers add together, whereas the latency they each observed does
//! not, and summing it would invent delay that nobody experienced. The second is
//! whether the quantity is shared out when work is spread across replicas. A
//! rate divides across a sharded fleet; the size of a request does not shrink
//! because there are more shards to send it to.
//!
//! # Extensive and intensive quantities
//!
//! The distinction is the familiar one from physics. An extensive quantity
//! scales with the size of the system it describes, so splitting the system
//! splits the quantity. An intensive quantity is a property of each unit of work
//! and is unchanged by how much of it there is.
//!
//! Getting this wrong is a quiet and expensive mistake. Treating a payload size
//! as extensive makes a fleet look as though adding shards shrinks its records;
//! treating a request rate as intensive makes every shard appear to carry the
//! whole load. Neither error announces itself in a result that still looks like
//! a number.
//!
//! # Which port may publish what
//!
//! A signal also says where a component is allowed to say it, and where it must.
//! Two types agree about a quantity only because they happen to use the same
//! name for it on the same side of themselves, and a type that publishes a rate
//! back toward its callers, or omits the latency its callers wait on, does not
//! fail — it quietly reads as though the missing figure were nought. Declaring
//! the rule here turns that into a diagnostic at load time and keeps every type
//! in the catalogue interchangeable with every other.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

/// How several arrivals of one signal combine into a single figure.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Aggregation {
    /// Arrivals add together, as rates and volumes do.
    #[default]
    Sum,
    /// The largest arrival wins, as with a latency that must all be waited out.
    Max,
    /// Arrivals average, for a quantity describing each unit of work.
    Mean,
    /// Arrivals multiply, as the success of dependencies that must all hold does.
    ///
    /// This is the reading that treats every dependency as hard: a caller
    /// needing three services succeeds only when all three do. A component that
    /// survives one of them failing is expressing something else, and should say
    /// so with a behaviour that turns a failure into a success rather than by
    /// combining differently.
    Product,
    /// The smallest arrival wins, as a rate limited by its narrowest stage is.
    Min,
}

impl Aggregation {
    /// The value that leaves a reader unaffected when nothing arrives.
    ///
    /// Zero is right for a rate, which nobody is offering, and wrong for a
    /// success rate: a component with no dependencies depends on nothing that
    /// could fail, and reading zero there would report every leaf of a design as
    /// a total outage and propagate that back to the caller.
    pub fn identity(self) -> f64 {
        match self {
            Self::Product => 1.0,
            // Nothing attached imposes no ceiling, so the limit is unbounded
            // rather than nought; reading zero would report an unattached port
            // as unable to carry anything at all.
            Self::Min => f64::INFINITY,
            Self::Sum | Self::Max | Self::Mean => 0.0,
        }
    }
}

/// Whether a port on one side of a component may publish a signal, and whether
/// it must.
///
/// Requiring a signal is what makes two component types substitutable. A caller
/// reads `in.<port>.latency` from whatever happens to be attached, and a callee
/// that never published one is indistinguishable from one that answers
/// instantly, so the omission reads as a design that is faster than it is.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Publication {
    /// Every port on this side must publish the signal.
    Required,
    /// A port on this side may publish the signal.
    #[default]
    Allowed,
    /// A port on this side may not publish the signal.
    Forbidden,
}

/// Where a component is allowed to publish a signal, and where it must.
///
/// Named for the side of the component rather than the direction of travel,
/// because that is how an author writes it: what an inbound port publishes goes
/// back to callers, and what an outbound port publishes goes on to dependencies.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Publishing {
    /// The rule for ports callers attach to, which answer them.
    #[serde(rename = "in", default)]
    pub inbound: Publication,
    /// The rule for ports dependencies attach to, which are asked by them.
    #[serde(rename = "out", default)]
    pub outbound: Publication,
}

/// One quantity that may travel along a relationship.
///
/// A signal carries no direction of its own. Which way it travels is settled by
/// the port publishing it: an inbound port publishes toward callers and an
/// outbound port toward dependencies, so the same `payload` names a request body
/// in one place and a reply body in the other without needing two names. Which
/// sides may say it at all, and which must, is declared here rather than left to
/// each type to observe.
///
/// A signal a manifest introduces without declaring here falls back to adding
/// across arrivals and not dividing across replicas, which is the safe reading:
/// it may overstate the load on one replica, and overstating a bottleneck is
/// recoverable in a way that missing one is not. It is publishable from either
/// side, because nothing else knows what a project's own vocabulary means.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Signal {
    /// Unit annotation the quantity carries.
    pub unit: String,
    /// What the quantity measures.
    #[serde(default)]
    pub summary: String,
    /// How several arrivals combine.
    #[serde(default)]
    pub aggregate: Aggregation,
    /// Which of a component's sides may publish the quantity, and which must.
    #[serde(default)]
    pub publish: Publishing,
    /// Whether the quantity is shared out across replicas.
    ///
    /// True for quantities that scale with the size of the system, such as a
    /// request rate. False for quantities describing each unit of work, such as
    /// a payload size or an observed latency.
    #[serde(default)]
    pub extensive: bool,
    /// What the quantity reads where nothing arrives carrying it.
    ///
    /// Defaults to the identity of the aggregation, which is what a quantity
    /// nobody is contributing to should read: nought for a rate, one for a
    /// success. Stated explicitly only where a signal describes a convention
    /// rather than a flow, and where the convention is not an identity — a share
    /// that assumes a half, say, which no aggregation could produce from
    /// nothing.
    #[serde(default)]
    pub rest: Option<f64>,
}

impl Signal {
    /// What this quantity reads where nothing arrives carrying it.
    pub fn rest(&self) -> f64 {
        self.rest.unwrap_or_else(|| self.aggregate.identity())
    }
}

/// The signals shipped with the tool, keyed by name.
pub(crate) fn builtin_signals() -> &'static BTreeMap<String, Signal> {
    static SHIPPED: LazyLock<BTreeMap<String, Signal>> = LazyLock::new(|| {
        serde_yaml_ng::from_str(include_str!("catalogue/signals.yaml"))
            .expect("the shipped signal vocabulary is valid")
    });
    &SHIPPED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_vocabulary_loads() {
        let signals = builtin_signals();
        for name in ["rate", "latency", "payload"] {
            assert!(signals.contains_key(name), "missing '{name}'");
        }
    }

    #[test]
    fn rates_are_extensive_and_add() {
        let signals = builtin_signals();
        assert!(signals["rate"].extensive);
        assert_eq!(signals["rate"].aggregate, Aggregation::Sum);
    }

    #[test]
    fn per_request_quantities_are_intensive() {
        // Sharding a fleet does not shrink the requests it serves, nor shorten
        // the time any one of them took.
        let signals = builtin_signals();
        assert!(!signals["payload"].extensive);
        assert!(!signals["latency"].extensive);
    }

    #[test]
    fn latency_does_not_sum_across_callers() {
        // Two callers each waiting a second is one second of waiting apiece,
        // not two.
        assert_eq!(builtin_signals()["latency"].aggregate, Aggregation::Max);
    }

    #[test]
    fn every_signal_documents_itself() {
        for (name, signal) in builtin_signals() {
            assert!(!signal.summary.trim().is_empty(), "'{name}' has no summary");
            assert!(!signal.unit.trim().is_empty(), "'{name}' has no unit");
        }
    }

    #[test]
    fn signals_travelling_one_way_are_forbidden_the_other() {
        let signals = builtin_signals();
        assert_eq!(signals["rate"].publish.inbound, Publication::Forbidden);
        assert_eq!(signals["success"].publish.outbound, Publication::Forbidden);
        assert_eq!(signals["capacity"].publish.outbound, Publication::Forbidden);
    }

    #[test]
    fn a_payload_may_be_published_from_either_side() {
        // The size of a request and the size of its reply are the same quantity
        // read from opposite ends, which is why one name serves for both.
        let payload = &builtin_signals()["payload"];
        assert_eq!(payload.publish.inbound, Publication::Allowed);
        assert_eq!(payload.publish.outbound, Publication::Allowed);
    }

    #[test]
    fn the_engine_keeps_peers_to_itself() {
        let peers = &builtin_signals()["peers"];
        assert_eq!(peers.publish.inbound, Publication::Forbidden);
        assert_eq!(peers.publish.outbound, Publication::Forbidden);
    }

    #[test]
    fn a_signal_a_project_invents_may_be_published_from_either_side() {
        let invented = Signal::default();
        assert_eq!(invented.publish.inbound, Publication::Allowed);
        assert_eq!(invented.publish.outbound, Publication::Allowed);
    }
}
