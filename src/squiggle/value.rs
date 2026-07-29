//! Runtime values produced by Squiggle evaluation.

use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    fmt,
    hash::{BuildHasherDefault, Hasher},
    rc::Rc,
};

use chrono::{DateTime, Datelike, NaiveDate, Utc};

use crate::profile::count;

use super::{
    ast::{Expression, Parameter},
    distribution::Distribution,
};

/// A dynamically typed Squiggle value.
#[derive(Clone, Debug)]
pub enum Value {
    /// IEEE-754 scalar number.
    Number(f64),
    /// Boolean truth value.
    Boolean(bool),
    /// UTF-8 string.
    String(String),
    /// Ordered heterogeneous collection.
    Array(Vec<Value>),
    /// String-keyed insertion-independent dictionary.
    ///
    /// Shared rather than owned. A dictionary is how a solve carries a
    /// component's inbound flows and how the standard library groups
    /// `Little`, `Queue` and the rest, so naming one is the ordinary way to
    /// reach a field. Reading a name copies the value it found, and copying a
    /// map copied every key and every entry beneath it: reaching
    /// `in.requests.rate` rebuilt two maps and three strings to read one
    /// number. Sharing the map makes that a reference count, and writing to one
    /// still copies, so a value is never seen to change under a holder.
    Dictionary(Rc<BTreeMap<String, Value>>),
    /// Scalar probability distribution.
    Distribution(Distribution),
    /// UTC calendar date represented at midnight.
    Date(DateValue),
    /// Signed elapsed time.
    Duration(DurationValue),
    /// Runtime argument-validation domain.
    Domain(Domain),
    /// Builtin or lexically scoped callable.
    Function(Function),
    /// Absence of a module result.
    Void,
}

impl Value {
    /// Wraps an owned map as a shared dictionary value.
    pub fn dictionary(entries: BTreeMap<String, Value>) -> Self {
        Self::Dictionary(Rc::new(entries))
    }

    /// Returns the stable runtime type name used in diagnostics.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Number(_) => "Number",
            Self::Boolean(_) => "Boolean",
            Self::String(_) => "String",
            Self::Array(_) => "Array",
            Self::Dictionary(_) => "Dictionary",
            Self::Distribution(_) => "Distribution",
            Self::Date(_) => "Date",
            Self::Duration(_) => "Duration",
            Self::Domain(_) => "Domain",
            Self::Function(_) => "Function",
            Self::Void => "Void",
        }
    }

    /// Borrows this value as a number.
    pub fn as_number(&self) -> Option<f64> {
        if let Self::Number(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    /// Borrows this value as a probability distribution.
    pub fn as_distribution(&self) -> Option<&Distribution> {
        if let Self::Distribution(value) = self {
            Some(value)
        } else {
            None
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Number(left), Self::Number(right)) => left == right,
            (Self::Boolean(left), Self::Boolean(right)) => left == right,
            (Self::String(left), Self::String(right)) => left == right,
            (Self::Array(left), Self::Array(right)) => left == right,
            (Self::Dictionary(left), Self::Dictionary(right)) => {
                Rc::ptr_eq(left, right) || left == right
            }
            (Self::Distribution(left), Self::Distribution(right)) => left == right,
            (Self::Date(left), Self::Date(right)) => left == right,
            (Self::Duration(left), Self::Duration(right)) => left == right,
            (Self::Domain(left), Self::Domain(right)) => left == right,
            (Self::Void, Self::Void) => true,
            (Self::Function(left), Self::Function(right)) => left.identity() == right.identity(),
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(value) => write!(formatter, "{value}"),
            Self::Boolean(value) => write!(formatter, "{value}"),
            Self::String(value) => write!(formatter, "{value}"),
            Self::Array(values) => write!(
                formatter,
                "[{}]",
                values
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Dictionary(values) => write!(
                formatter,
                "{{{}}}",
                values
                    .iter()
                    .map(|(key, value)| format!("{key}: {value}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Distribution(value) => write!(formatter, "<{} distribution>", value.family()),
            Self::Date(value) => write!(formatter, "{value}"),
            Self::Duration(value) => write!(formatter, "{} days", value.as_days()),
            Self::Domain(value) => write!(formatter, "{value}"),
            Self::Function(value) => write!(formatter, "<function {}>", value.name()),
            Self::Void => formatter.write_str("void"),
        }
    }
}

/// A bounded scalar domain used by function parameter annotations.
#[derive(Clone, Debug, PartialEq)]
pub enum Domain {
    /// Inclusive numeric interval.
    NumberRange {
        /// Inclusive lower bound.
        minimum: f64,
        /// Inclusive upper bound.
        maximum: f64,
    },
    /// Inclusive UTC date interval.
    DateRange {
        /// Inclusive earliest date.
        minimum: DateValue,
        /// Inclusive latest date.
        maximum: DateValue,
    },
}

impl Domain {
    /// Returns whether `value` belongs to this domain.
    pub fn contains(&self, value: &Value) -> bool {
        match (self, value) {
            (Self::NumberRange { minimum, maximum }, Value::Number(value)) => {
                (*minimum..=*maximum).contains(value)
            }
            (Self::DateRange { minimum, maximum }, Value::Date(value)) => {
                (*minimum..=*maximum).contains(value)
            }
            _ => false,
        }
    }
}

impl fmt::Display for Domain {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NumberRange { minimum, maximum } => write!(formatter, "{minimum} to {maximum}"),
            Self::DateRange { minimum, maximum } => write!(formatter, "{minimum} to {maximum}"),
        }
    }
}

/// A UTC calendar date stored as Unix milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct DateValue(f64);

impl DateValue {
    /// Creates a date from a finite Unix timestamp in seconds.
    pub fn from_unix_seconds(seconds: f64) -> Result<Self, String> {
        if !seconds.is_finite() {
            return Err("Unix timestamp must be finite".into());
        }
        let milliseconds = seconds * 1_000.0;
        milliseconds
            .is_finite()
            .then_some(Self(milliseconds))
            .ok_or_else(|| "Unix timestamp is outside the supported range".into())
    }

    /// Creates a midnight UTC date from Gregorian components.
    pub fn from_ymd(year: i32, month: u32, day: u32) -> Result<Self, String> {
        let date = NaiveDate::from_ymd_opt(year, month, day)
            .ok_or_else(|| "invalid Gregorian date".to_owned())?;
        let midnight = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| "failed to construct midnight for Gregorian date".to_owned())?;
        Ok(Self(midnight.and_utc().timestamp_millis() as f64))
    }

    /// Parses an ISO `YYYY-MM-DD` date.
    pub fn parse(value: &str) -> Result<Self, String> {
        let date =
            NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|error| error.to_string())?;
        Self::from_ymd(
            chrono::Datelike::year(&date),
            chrono::Datelike::month(&date),
            chrono::Datelike::day(&date),
        )
    }

    /// Returns Unix time in seconds.
    pub fn unix_seconds(self) -> f64 {
        self.0 / 1_000.0
    }

    pub(crate) fn add(self, duration: DurationValue) -> Self {
        Self(self.0 + duration.0)
    }

    pub(crate) fn subtract(self, other: Self) -> DurationValue {
        DurationValue(self.0 - other.0)
    }
}

impl fmt::Display for DateValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(date) = DateTime::<Utc>::from_timestamp_millis(self.0 as i64) else {
            return formatter.write_str("invalid date");
        };
        let date = date.date_naive();
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            date.year(),
            date.month(),
            date.day()
        )
    }
}

/// A signed duration stored in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct DurationValue(f64);

impl DurationValue {
    /// Creates a finite duration from milliseconds.
    pub fn from_milliseconds(milliseconds: f64) -> Result<Self, String> {
        milliseconds
            .is_finite()
            .then_some(Self(milliseconds))
            .ok_or_else(|| "duration must be finite".into())
    }

    /// Creates a duration from minutes.
    pub fn from_minutes(value: f64) -> Result<Self, String> {
        Self::from_milliseconds(value * 60_000.0)
    }
    /// Creates a duration from hours.
    pub fn from_hours(value: f64) -> Result<Self, String> {
        Self::from_milliseconds(value * 3_600_000.0)
    }
    /// Creates a duration from days.
    pub fn from_days(value: f64) -> Result<Self, String> {
        Self::from_milliseconds(value * 86_400_000.0)
    }
    /// Creates a duration from 365.25-day years.
    pub fn from_years(value: f64) -> Result<Self, String> {
        Self::from_days(value * 365.25)
    }
    /// Returns this duration in minutes.
    pub fn as_minutes(self) -> f64 {
        self.0 / 60_000.0
    }
    /// Returns this duration in hours.
    pub fn as_hours(self) -> f64 {
        self.0 / 3_600_000.0
    }
    /// Returns this duration in days.
    pub fn as_days(self) -> f64 {
        self.0 / 86_400_000.0
    }
    /// Returns this duration in 365.25-day years.
    pub fn as_years(self) -> f64 {
        self.as_days() / 365.25
    }
    pub(crate) fn milliseconds(self) -> f64 {
        self.0
    }
}

/// A callable builtin or user-defined closure.
#[derive(Clone, Debug)]
pub struct Function(pub(crate) Rc<FunctionKind>);

impl Function {
    /// Returns the diagnostic name of this function.
    pub fn name(&self) -> &str {
        match self.0.as_ref() {
            FunctionKind::Builtin(name) => name,
            FunctionKind::User { name, .. } => name.as_deref().unwrap_or("anonymous"),
        }
    }

    /// Returns the declared arity for user functions, if statically known.
    pub fn arity(&self) -> Option<usize> {
        match self.0.as_ref() {
            FunctionKind::Builtin(_) => None,
            FunctionKind::User { parameters, .. } => Some(parameters.len()),
        }
    }

    pub(crate) fn builtin(name: &'static str) -> Self {
        Self(Rc::new(FunctionKind::Builtin(name)))
    }

    pub(crate) fn user(
        name: Option<String>,
        parameters: Vec<Parameter>,
        body: Expression,
        environment: Environment,
    ) -> Self {
        Self(Rc::new(FunctionKind::User {
            name,
            parameters,
            body,
            environment,
        }))
    }

    fn identity(&self) -> *const FunctionKind {
        Rc::as_ptr(&self.0)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum FunctionKind {
    Builtin(&'static str),
    User {
        name: Option<String>,
        parameters: Vec<Parameter>,
        body: Expression,
        environment: Environment,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct Environment(Rc<Frame>);

/// Names bound in one scope.
///
/// Hashed rather than ordered. Nothing reads a scope in order — a module's
/// exports are gathered from its statements, not from the frame — while every
/// name in every expression is resolved through here, and an ordered map spends
/// that lookup comparing the name against several others before it finds the one
/// it wants. The standard library alone binds over a hundred names in the frame
/// every builtin call has to reach, so those comparisons were the largest single
/// cost in evaluating a program.
type Names = HashMap<String, Value, BuildHasherDefault<NameHasher>>;

/// A hasher for short identifiers.
///
/// The default hasher is chosen to make collisions hard to provoke deliberately,
/// which is the wrong trade here: the keys are identifiers written by whoever
/// wrote the model, and the map lives for one evaluation. This is FxHash, the
/// multiply-and-rotate mix used by the Rust compiler for its own symbol tables,
/// which costs a few instructions per eight bytes of name.
#[derive(Default)]
struct NameHasher(u64);

impl Hasher for NameHasher {
    fn write(&mut self, bytes: &[u8]) {
        const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            self.mix(u64::from_le_bytes(chunk.try_into().expect("eight bytes")));
        }
        let mut tail = 0u64;
        for (index, byte) in chunks.remainder().iter().enumerate() {
            tail |= u64::from(*byte) << (index * 8);
        }
        self.mix(tail);
        self.0 = self.0.wrapping_mul(SEED);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

impl NameHasher {
    #[inline]
    fn mix(&mut self, word: u64) {
        const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

#[derive(Debug)]
struct Frame {
    parent: Option<Environment>,
    values: RefCell<Names>,
}

impl Environment {
    pub(crate) fn root() -> Self {
        Self(Rc::new(Frame {
            parent: None,
            values: RefCell::new(Names::default()),
        }))
    }

    pub(crate) fn child(&self) -> Self {
        Self(Rc::new(Frame {
            parent: Some(self.clone()),
            values: RefCell::new(Names::default()),
        }))
    }

    pub(crate) fn snapshot(&self) -> Self {
        Self(Rc::new(Frame {
            parent: self.0.parent.as_ref().map(Environment::snapshot),
            values: RefCell::new(self.0.values.borrow().clone()),
        }))
    }

    pub(crate) fn define(&self, name: impl Into<String>, value: Value) {
        self.0.values.borrow_mut().insert(name.into(), value);
    }

    /// Writes `value` into this frame, keeping the existing key when there is one.
    ///
    /// [`Environment::define`] owns its name, so re-binding through it allocates a
    /// fresh key every time. A caller that writes the same names over and over --
    /// one node equation evaluated for every period of every draw -- can reuse the
    /// frame instead and pay only for the value.
    pub(crate) fn rebind(&self, name: &str, value: Value) {
        let mut values = self.0.values.borrow_mut();
        match values.get_mut(name) {
            Some(slot) => *slot = value,
            None => {
                values.insert(name.to_owned(), value);
            }
        }
    }

    pub(crate) fn get(&self, name: &str) -> Option<Value> {
        count!(Lookups);
        match self.0.values.borrow().get(name).cloned() {
            Some(value) => {
                count!(LookupEntries, copied(&value));
                Some(value)
            }
            None => self.0.parent.as_ref()?.get(name),
        }
    }
}

/// Counts the entries a lookup copied, so a costly name can be told from a cheap one.
#[cfg(feature = "profiling")]
fn copied(value: &Value) -> u64 {
    match value {
        Value::Dictionary(entries) => entries.values().map(copied).sum::<u64>() + 1,
        Value::Array(items) => items.iter().map(copied).sum::<u64>() + 1,
        _ => 1,
    }
}
