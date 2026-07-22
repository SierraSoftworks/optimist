use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Constraint {
    Any,
    Number,
    Integer,
    NonNegativeInteger,
    Boolean,
    String,
    Array(Box<Constraint>),
    Dictionary,
    Distribution,
    Function,
    Date,
    Duration,
    Union(Vec<Constraint>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ParameterConstraint {
    pub(crate) name: &'static str,
    pub(crate) constraint: Constraint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BuiltinSignature {
    pub(crate) names: Vec<&'static str>,
    pub(crate) parameters: Vec<ParameterConstraint>,
    pub(crate) variadic: Option<ParameterConstraint>,
}

impl fmt::Display for Constraint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Any => formatter.write_str("*"),
            Self::Number => formatter.write_str("Number"),
            Self::Integer => formatter.write_str("Integer"),
            Self::NonNegativeInteger => formatter.write_str("NonNegativeInteger"),
            Self::Boolean => formatter.write_str("Boolean"),
            Self::String => formatter.write_str("String"),
            Self::Array(element) => write!(formatter, "[{element}]"),
            Self::Dictionary => formatter.write_str("Dictionary"),
            Self::Distribution => formatter.write_str("Distribution"),
            Self::Function => formatter.write_str("Function"),
            Self::Date => formatter.write_str("Date"),
            Self::Duration => formatter.write_str("Duration"),
            Self::Union(values) => formatter.write_str(
                &values
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" | "),
            ),
        }
    }
}
