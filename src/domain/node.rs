use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

use super::{Duration, EntityId, Estimate, Money, NormalizedState, Probability};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Outcome,
    Metric,
    Factor,
    Intervention,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Evidence {
    pub id: u64,
    pub revision: u64,
    pub summary: String,
    pub source: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeDirection {
    Maximize,
    Minimize,
    TargetRange,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Outcome {
    pub direction: OutcomeDirection,
    pub current: Option<Estimate<NormalizedState>>,
    pub desired: Option<Estimate<NormalizedState>>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Metric {
    pub unit: String,
    pub aggregation: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Factor {
    pub current: Option<Estimate<NormalizedState>>,
    pub desired: Option<Estimate<NormalizedState>>,
    pub controllable: bool,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CostEstimate {
    pub dimension: String,
    pub value: Estimate<Money>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Intervention {
    #[serde(default)]
    pub costs: Vec<CostEstimate>,
    pub duration: Option<Estimate<Duration>>,
    pub probability_of_success: Option<Estimate<Probability>>,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "properties", rename_all = "snake_case")]
pub enum NodePayload {
    Outcome(Outcome),
    Metric(Metric),
    Factor(Factor),
    Intervention(Intervention),
}

impl NodePayload {
    pub const fn kind(&self) -> NodeKind {
        match self {
            Self::Outcome(_) => NodeKind::Outcome,
            Self::Metric(_) => NodeKind::Metric,
            Self::Factor(_) => NodeKind::Factor,
            Self::Intervention(_) => NodeKind::Intervention,
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum NodeError {
    #[error("a node name cannot be empty")]
    EmptyName,
    #[error("a node title cannot be empty")]
    EmptyTitle,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Node {
    pub id: EntityId,
    pub revision: u64,
    pub name: String,
    pub normalized_name: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub payload: NodePayload,
}

impl Node {
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
            payload,
        })
    }

    pub const fn kind(&self) -> NodeKind {
        self.payload.kind()
    }
}

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
    use super::{Factor, Node, NodeKind, NodePayload, normalize_name};
    use crate::domain::{Distribution, EntityId, EstimateId};

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
}
