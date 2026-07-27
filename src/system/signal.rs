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

use std::collections::BTreeMap;

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
}

/// One quantity that may travel along a relationship.
///
/// A signal a manifest introduces without declaring here falls back to adding
/// across arrivals and not dividing across replicas, which is the safe reading:
/// it may overstate the load on one replica, and overstating a bottleneck is
/// recoverable in a way that missing one is not.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Signal {
    /// Unit annotation the quantity carries.
    pub unit: String,
    /// What the quantity measures.
    #[serde(default)]
    pub summary: String,
    /// How several arrivals combine.
    #[serde(default)]
    pub aggregate: Aggregation,
    /// Whether the quantity is shared out across replicas.
    ///
    /// True for quantities that scale with the size of the system, such as a
    /// request rate. False for quantities describing each unit of work, such as
    /// a payload size or an observed latency.
    #[serde(default)]
    pub extensive: bool,
}

/// The signals shipped with the tool, keyed by name.
pub(super) fn builtin_signals() -> BTreeMap<String, Signal> {
    serde_yaml_ng::from_str(include_str!("catalogue/signals.yaml"))
        .expect("the shipped signal vocabulary is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_vocabulary_loads() {
        let signals = builtin_signals();
        for name in ["rate", "added_latency", "payload"] {
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
        assert!(!signals["added_latency"].extensive);
    }

    #[test]
    fn latency_does_not_sum_across_callers() {
        // Two callers each waiting a second is one second of waiting apiece,
        // not two.
        assert_eq!(
            builtin_signals()["added_latency"].aggregate,
            Aggregation::Max
        );
    }

    #[test]
    fn every_signal_documents_itself() {
        for (name, signal) in builtin_signals() {
            assert!(!signal.summary.trim().is_empty(), "'{name}' has no summary");
            assert!(!signal.unit.trim().is_empty(), "'{name}' has no unit");
        }
    }
}
