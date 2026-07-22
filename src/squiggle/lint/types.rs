use std::{collections::BTreeMap, fmt};

use crate::squiggle::ast::UnitType;

use super::Constraint;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Unit(pub(super) BTreeMap<String, f64>);

impl Unit {
    pub(super) fn from_ast(unit: &UnitType) -> Self {
        match unit {
            UnitType::Factor { name, exponent } => {
                if name.parse::<f64>().is_ok() {
                    Self::default()
                } else {
                    Self([(name.clone(), *exponent)].into())
                }
            }
            UnitType::Product(left, right) => {
                Self::from_ast(left).combine(&Self::from_ast(right), 1.0)
            }
            UnitType::Ratio(left, right) => {
                Self::from_ast(left).combine(&Self::from_ast(right), -1.0)
            }
        }
    }

    pub(super) fn combine(&self, other: &Self, direction: f64) -> Self {
        let mut result = self.0.clone();
        for (name, exponent) in &other.0 {
            *result.entry(name.clone()).or_default() += direction * exponent;
        }
        result.retain(|_, exponent| exponent.abs() > f64::EPSILON);
        Self(result)
    }

    pub(super) fn pow(&self, exponent: f64) -> Self {
        Self(
            self.0
                .iter()
                .map(|(name, power)| (name.clone(), power * exponent))
                .collect(),
        )
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return formatter.write_str("dimensionless");
        }
        formatter.write_str(
            &self
                .0
                .iter()
                .map(|(name, power)| {
                    if (*power - 1.0).abs() <= f64::EPSILON {
                        name.clone()
                    } else {
                        format!("{name}^{power}")
                    }
                })
                .collect::<Vec<_>>()
                .join("*"),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct FunctionType {
    pub(super) parameters: Vec<Type>,
    pub(super) result: Box<Type>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Type {
    Unknown,
    Number { literal: Option<f64>, unit: Unit },
    Boolean,
    String,
    Array(Box<Type>),
    Dictionary(BTreeMap<String, Type>),
    Distribution(Unit),
    Function(FunctionType),
    Builtin(String),
    Date,
    Duration,
    Domain,
    Union(Vec<Type>),
}

impl Type {
    pub(super) fn number(literal: Option<f64>) -> Self {
        Self::Number {
            literal,
            unit: Unit::default(),
        }
    }

    pub(super) fn display_name(&self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Number { .. } => "Number",
            Self::Boolean => "Boolean",
            Self::String => "String",
            Self::Array(_) => "Array",
            Self::Dictionary(_) => "Dictionary",
            Self::Distribution(_) => "Distribution",
            Self::Function(_) | Self::Builtin(_) => "Function",
            Self::Date => "Date",
            Self::Duration => "Duration",
            Self::Domain => "Domain",
            Self::Union(_) => "Union",
        }
    }

    pub(super) fn accepts(&self, constraint: &Constraint) -> bool {
        if let Self::Union(values) = self {
            return values.iter().all(|value| value.accepts(constraint));
        }
        match constraint {
            Constraint::Any => true,
            Constraint::Number => matches!(self, Self::Unknown | Self::Number { .. }),
            Constraint::Integer => match self {
                Self::Unknown | Self::Number { literal: None, .. } => true,
                Self::Number {
                    literal: Some(value),
                    ..
                } => value.is_finite() && value.fract() == 0.0,
                _ => false,
            },
            Constraint::NonNegativeInteger => match self {
                Self::Unknown | Self::Number { literal: None, .. } => true,
                Self::Number {
                    literal: Some(value),
                    ..
                } => value.is_finite() && value.fract() == 0.0 && *value >= 0.0,
                _ => false,
            },
            Constraint::Boolean => matches!(self, Self::Unknown | Self::Boolean),
            Constraint::String => matches!(self, Self::Unknown | Self::String),
            Constraint::Array(element) => match self {
                Self::Unknown => true,
                Self::Array(value) => value.accepts(element),
                _ => false,
            },
            Constraint::Dictionary => matches!(self, Self::Unknown | Self::Dictionary(_)),
            Constraint::Distribution => matches!(self, Self::Unknown | Self::Distribution(_)),
            Constraint::Function => {
                matches!(self, Self::Unknown | Self::Function(_) | Self::Builtin(_))
            }
            Constraint::Date => matches!(self, Self::Unknown | Self::Date),
            Constraint::Duration => matches!(self, Self::Unknown | Self::Duration),
            Constraint::Union(values) => values.iter().any(|value| self.accepts(value)),
        }
    }
}
