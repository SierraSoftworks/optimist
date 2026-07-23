use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

use super::{
    Duration, EntityId, Estimate, Money, NormalizedState, Probability, QuantityDefinition,
    QuantityError, QuantityState, QuantitySupport, QuantityValue,
};

/// The four structural concepts rendered as vertices in an Optimist graph.
///
/// Observations, estimates, costs, and other parent-owned values deliberately do
/// not appear here; they are embedded in the appropriate node or edge payload to
/// keep traversal focused on causal structure.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// An observed or desired system result whose utility guides prioritization.
    Outcome,
    /// A defined measurement used to observe an outcome or factor.
    Metric,
    /// A condition which causally influences another factor, metric, or outcome.
    Factor,
    /// A concrete investable action expected to change one or more factors.
    Intervention,
}

/// Qualitative evidence or a symptom embedded in its subject node.
///
/// Evidence is embedded because it has one structural owner and should appear in
/// that owner's details pane rather than complicating graph traversal.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Evidence {
    /// Aggregate-local identifier used for corrections and collaboration conflicts.
    pub id: u64,
    /// Optimistic-concurrency revision of this evidence record.
    pub revision: u64,
    /// Concise Markdown-compatible description of the observation or symptom.
    pub summary: String,
    /// Optional source citation, URL, system, or person from which the evidence came.
    pub source: Option<String>,
}

/// Describes how movement in an outcome maps to scenario utility.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeDirection {
    /// Larger outcome values are preferred.
    Maximize,
    /// Smaller outcome values are preferred.
    Minimize,
    /// Values inside a separately configured target interval are preferred.
    TargetRange,
}

/// Outcome-specific data embedded in an [`NodePayload::Outcome`] vertex.
///
/// Current and desired states are probability distributions rather than point
/// claims, allowing rankings to expose uncertainty about both baseline and goal.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Outcome {
    /// Utility direction used when converting outcome movement into benefit.
    pub direction: OutcomeDirection,
    /// Optional uncertain normalized baseline for the outcome.
    pub current: Option<Estimate<NormalizedState>>,
    /// Optional uncertain normalized target for the outcome.
    pub desired: Option<Estimate<NormalizedState>>,
    /// Qualitative evidence and symptoms directly owned by this outcome.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// Defines a reusable measurement concept such as deployment frequency or latency.
///
/// Actual readings belong to the metric's `measures` edge for a specific subject,
/// because one metric may measure several factors or outcomes independently.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Metric {
    /// Native-unit operational definition shared by estimates and observations.
    #[serde(flatten)]
    pub quantity: QuantityDefinition,
    /// Optional uncertain current or forecast value in [`QuantityDefinition::unit`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<Estimate<QuantityValue>>,
}

impl Metric {
    /// Creates a metric compatible with the legacy unit and aggregation fields.
    pub fn new(
        unit: impl Into<String>,
        aggregation: Option<String>,
    ) -> Result<Self, QuantityError> {
        Self::with_quantity(
            QuantityDefinition::new(unit, aggregation, QuantitySupport::Real)?,
            None,
        )
    }

    /// Creates a measured quantity after checking its current estimate's support.
    pub fn with_quantity(
        quantity: QuantityDefinition,
        current: Option<Estimate<QuantityValue>>,
    ) -> Result<Self, QuantityError> {
        let quantity = quantity.validated()?;
        if current
            .as_ref()
            .is_some_and(|value| !quantity.accepts(&value.distribution))
        {
            return Err(QuantityError::EstimateOutsideSupport);
        }
        Ok(Self { quantity, current })
    }

    fn validated(self) -> Result<Self, QuantityError> {
        Self::with_quantity(self.quantity, self.current)
    }
}

#[derive(Deserialize)]
struct MetricWire {
    #[serde(flatten)]
    quantity: QuantityDefinition,
    #[serde(default)]
    current: Option<Estimate<QuantityValue>>,
}

impl<'de> Deserialize<'de> for Metric {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = MetricWire::deserialize(deserializer)?;
        Self::with_quantity(value.quantity, value.current).map_err(de::Error::custom)
    }
}

/// Factor-specific state embedded in a causal graph vertex.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Factor {
    /// Optional uncertain normalized baseline for the factor.
    pub current: Option<Estimate<NormalizedState>>,
    /// Optional uncertain normalized target state for the factor.
    pub desired: Option<Estimate<NormalizedState>>,
    /// Whether a team can act on the factor directly, used to filter leverage points.
    pub controllable: bool,
    /// Qualitative evidence and symptoms directly owned by this factor.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// One dimension of an intervention's uncertain intrinsic cost.
///
/// Keeping dimensions separate avoids silently adding incomparable quantities such
/// as currency, engineering effort, and operational risk.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CostEstimate {
    /// Project-defined dimension name, for example `usd` or `engineer_days`.
    pub dimension: String,
    /// Non-negative probability distribution for this cost dimension.
    pub value: Estimate<Money>,
}

/// Intervention-specific investment data embedded in an actionable vertex.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Intervention {
    /// Independent cost dimensions considered by Pareto optimization.
    #[serde(default)]
    pub costs: Vec<CostEstimate>,
    /// Optional uncertain elapsed time before the intervention is complete.
    pub duration: Option<Estimate<Duration>>,
    /// Optional probability that implementation produces its modelled changes.
    pub probability_of_success: Option<Estimate<Probability>>,
    /// Verifiable conditions used to decide whether implementation is complete.
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
}

/// Type-safe payload for one of Optimist's four graph vertex kinds.
///
/// The tagged enum keeps kind-specific fields out of arbitrary metadata and makes
/// invalid combinations impossible to deserialize.
///
/// ```
/// use optimist::domain::{Factor, NodePayload};
///
/// let payload = NodePayload::Factor(Factor {
///     current: None,
///     desired: None,
///     controllable: true,
///     evidence: vec![],
/// });
/// ```
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "properties", rename_all = "snake_case")]
pub enum NodePayload {
    /// Fields owned by an outcome vertex.
    Outcome(Outcome),
    /// Fields owned by a metric vertex.
    Metric(Metric),
    /// Fields owned by a factor vertex.
    Factor(Factor),
    /// Fields owned by an intervention vertex.
    Intervention(Intervention),
}

impl NodePayload {
    /// Returns the vertex kind implied by this payload's enum variant.
    ///
    /// Storage uses this value as the coarse IndraDB vertex type, while the full
    /// payload remains the source of truth for type-specific data.
    pub const fn kind(&self) -> NodeKind {
        match self {
            Self::Outcome(_) => NodeKind::Outcome,
            Self::Metric(_) => NodeKind::Metric,
            Self::Factor(_) => NodeKind::Factor,
            Self::Intervention(_) => NodeKind::Intervention,
        }
    }

    fn validated(self) -> Result<Self, QuantityError> {
        match self {
            Self::Metric(value) => value.validated().map(Self::Metric),
            value => Ok(value),
        }
    }
}

/// Validation failures returned while constructing a graph node.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum NodeError {
    /// The unique name becomes empty after Unicode and whitespace normalization.
    #[error("a node name cannot be empty")]
    EmptyName,
    /// The human-facing title contains no visible text.
    #[error("a node title cannot be empty")]
    EmptyTitle,
    /// A state-bearing metric has an invalid quantity definition or estimate.
    #[error(transparent)]
    Quantity(#[from] QuantityError),
    /// Native quantity state is supported only by factors and outcomes.
    #[error("native quantity state is supported only by factor and outcome nodes")]
    NativeStateUnsupported,
    /// Native and legacy standardized estimates cannot coexist on one node.
    #[error("native quantity state cannot coexist with legacy standardized estimates")]
    MixedStateStorage,
}

/// A structural causal graph vertex and its embedded aggregate data.
///
/// `name` is project-unique and agent-addressable; `title` is presentation text.
/// Call [`Node::new`] so the stored normalized name is derived consistently.
///
/// ```
/// use optimist::domain::{EntityId, Factor, Node, NodePayload};
///
/// let node = Node::new(
///     EntityId::new(0),
///     "github",
///     "GitHub",
///     NodePayload::Factor(Factor {
///         current: None,
///         desired: None,
///         controllable: false,
///         evidence: vec![],
///     }),
/// )?;
/// assert_eq!(node.normalized_name, "github");
/// # Ok::<(), optimist::domain::NodeError>(())
/// ```
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Node {
    /// Project-local short identifier used in API, CLI, and edge references.
    pub id: EntityId,
    /// Aggregate revision used for optimistic concurrency and stale-analysis detection.
    pub revision: u64,
    /// Project-unique semantic name used by people and agents to address the node.
    pub name: String,
    /// Canonical case-insensitive lookup key derived from [`Node::name`].
    pub normalized_name: String,
    /// Human-facing display label, which need not be unique.
    pub title: String,
    /// Rich Markdown explanation of meaning, boundaries, and assumptions.
    pub description: String,
    /// Additional project-unique names resolving to this same node.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Extensible non-structural JSON data not covered by the typed payload.
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
    /// Optional native-unit state for a factor or outcome; absent means legacy standardized state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_state: Option<QuantityState>,
    /// Strongly typed fields determined by this node's structural kind.
    pub payload: NodePayload,
}

impl Node {
    /// Constructs a node and derives its canonical lookup name.
    ///
    /// Repository insertion performs project-wide ID/name checks; this constructor
    /// performs aggregate-local validation and should be used instead of literals.
    pub fn new(
        id: EntityId,
        name: impl Into<String>,
        title: impl Into<String>,
        payload: NodePayload,
    ) -> Result<Self, NodeError> {
        let name = name.into();
        let normalized_name = normalize_name(&name);
        if normalized_name.is_empty() {
            return Err(NodeError::EmptyName);
        }

        let title = title.into();
        if title.trim().is_empty() {
            return Err(NodeError::EmptyTitle);
        }

        Ok(Self {
            id,
            revision: 0,
            name,
            normalized_name,
            title,
            description: String::new(),
            aliases: Vec::new(),
            metadata: BTreeMap::new(),
            native_state: None,
            payload: payload.validated()?,
        })
    }

    /// Returns the structural kind implied by this node's typed payload.
    pub const fn kind(&self) -> NodeKind {
        self.payload.kind()
    }

    pub(crate) fn validate_native_state(&self) -> Result<(), NodeError> {
        let Some(_) = &self.native_state else {
            return Ok(());
        };
        match &self.payload {
            NodePayload::Factor(value) => {
                if value.current.is_some() || value.desired.is_some() {
                    Err(NodeError::MixedStateStorage)
                } else {
                    Ok(())
                }
            }
            NodePayload::Outcome(value) => {
                if value.current.is_some() || value.desired.is_some() {
                    Err(NodeError::MixedStateStorage)
                } else {
                    Ok(())
                }
            }
            _ => Err(NodeError::NativeStateUnsupported),
        }
    }
}

/// Produces the canonical key used for case-insensitive name and alias uniqueness.
///
/// NFKC normalization combines compatibility-equivalent Unicode forms, lowercasing
/// removes case distinctions, and whitespace runs collapse to one ASCII space.
///
/// ```
/// use optimist::domain::normalize_name;
/// assert_eq!(normalize_name("  CAFÉ\tReliability "), "café reliability");
/// ```
pub fn normalize_name(value: &str) -> String {
    value
        .nfkc()
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::{Factor, Metric, Node, NodeKind, NodePayload, normalize_name};
    use crate::domain::{
        Distribution, EntityId, Estimate, EstimateId, QuantityDefinition, QuantityError,
        QuantityState, QuantitySupport, QuantityValue, Unit,
    };

    #[test]
    fn normalizes_case_whitespace_and_unicode_composition() {
        assert_eq!(normalize_name("  CAFÉ\tReliability "), "café reliability");
        assert_eq!(normalize_name("CAFE\u{301}"), "café");
    }

    #[test]
    fn payload_determines_node_kind() {
        let node = Node::new(
            EntityId::new(1),
            "delivery reliability",
            "Delivery reliability",
            NodePayload::Factor(Factor {
                current: None,
                desired: None,
                controllable: true,
                evidence: Vec::new(),
            }),
        )
        .expect("valid node");

        assert_eq!(node.kind(), NodeKind::Factor);
    }

    #[test]
    fn sample_state_estimate_is_constructible() {
        let estimate = crate::domain::Estimate::<crate::domain::NormalizedState>::new(
            EstimateId::new(1),
            Distribution::beta(2.0, 3.0).expect("valid beta"),
        );
        assert!(estimate.is_ok());
    }

    #[test]
    fn legacy_metrics_round_trip_without_new_default_fields() {
        let json = r#"{"unit":"days","aggregation":"p95 weekly"}"#;
        let metric = serde_json::from_str::<Metric>(json).unwrap();

        assert_eq!(metric.quantity.support, QuantitySupport::Real);
        assert_eq!(serde_json::to_string(&metric).unwrap(), json);
    }

    #[test]
    fn metric_estimates_must_fit_native_quantity_support() {
        let quantity = QuantityDefinition::new(
            "days",
            None,
            QuantitySupport::Bounded {
                lower: 0.0,
                upper: 10.0,
            },
        )
        .unwrap();
        let estimate = Estimate::<QuantityValue>::new(
            EstimateId::new(0),
            Distribution::normal(5.0, 2.0).unwrap(),
        )
        .unwrap();

        assert_eq!(
            Metric::with_quantity(quantity, Some(estimate)),
            Err(QuantityError::EstimateOutsideSupport)
        );
    }

    #[test]
    fn native_state_is_optional_and_validates_owner_support() {
        let legacy = Node::new(
            EntityId::new(0),
            "flow",
            "Flow",
            NodePayload::Factor(Factor {
                current: None,
                desired: None,
                controllable: false,
                evidence: vec![],
            }),
        )
        .unwrap();
        assert!(
            !serde_json::to_string(&legacy)
                .unwrap()
                .contains("native_state")
        );

        let quantity = QuantityDefinition::with_dimension(
            "days",
            Some(Unit::base("day").unwrap()),
            None,
            QuantitySupport::Bounded {
                lower: 0.0,
                upper: 10.0,
            },
        )
        .unwrap();
        let estimate =
            Estimate::<QuantityValue>::new(EstimateId::new(0), Distribution::point(12.0).unwrap())
                .unwrap();
        assert_eq!(
            QuantityState::new(quantity, Some(estimate), None),
            Err(QuantityError::EstimateOutsideSupport)
        );
    }
}
