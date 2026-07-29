//! Handing a value to another thread without making the runtime thread-safe.
//!
//! # What actually stops a value from being sent
//!
//! Only one thing. [`Value::Function`] holds a callable, and a callable holds the
//! scope it closed over: a reference-counted frame with interior mutability.
//! Making those atomic would put a lock on every name lookup in every pass of
//! every solve to buy something only the thread boundary needs.
//!
//! Everything else a solve produces is already transferable. Numbers, strings and
//! dates are plain data, and a [`Distribution`] holds its draws behind an
//! [`Arc`](std::sync::Arc). So the boundary needs a form of [`Value`] that cannot
//! hold a callable, rather than a rewrite of the runtime.
//!
//! # Why this is a type and not an `unsafe impl Send`
//!
//! Asserting that a particular value happens to hold no callables would be a
//! promise about a value, checked once, and silently broken by whoever adds the
//! next variant to [`Value`]. [`Transferred`] instead makes the promise
//! structurally: it has no callable variant, so the compiler derives [`Send`] and
//! [`Sync`] for it, and adding a variant to [`Value`] fails to compile here until
//! somebody decides what crossing a thread should mean for it.
//!
//! # Sharing survives the crossing
//!
//! Two references to one binding are the same distribution, and that shared
//! identity is what makes dependence work:
//!
//! ```text
//! x = normal(5, 1)
//! x - x            // exactly zero, not a distribution centred on zero
//! ```
//!
//! Both operands read one materialised sample set, so every draw cancels against
//! itself. A form that copied the draws structurally would produce two
//! independent `normal(5, 1)`s, and the difference would acquire a spread it has
//! no business having — quietly, wherever a quantity was used twice, which in a
//! system model is everywhere. Snapshotting clones the handle rather than the
//! draws, so what shared a sample set before the crossing shares one after it.
//!
//! Squiggle's own serializer solves the same problem by interning nodes in a
//! bundle and referring to them by index. That machinery earns its keep when the
//! destination is bytes, where no pointer survives. Within one process an
//! [`Arc`](std::sync::Arc) already says what the interning table would have to
//! reconstruct, so this carries the handle and stays a tree. Going to bytes later
//! means adding the intern table, not revisiting this.

use std::collections::BTreeMap;

use super::{
    Distribution, Value,
    value::{DateValue, Domain, DurationValue},
};

/// Why a value could not be handed to another thread.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotError {
    /// A callable was found in the value.
    ///
    /// Solved quantities are numeric, so this reports an attempt to move
    /// something that was never going to survive the crossing rather than
    /// dropping it and leaving a hole in the result.
    NotTransferable {
        /// Type name of the value that could not be sent.
        found: &'static str,
    },
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self::NotTransferable { found } = self;
        write!(
            formatter,
            "a {found} closes over its defining scope and cannot be moved between threads"
        )
    }
}

impl std::error::Error for SnapshotError {}

/// A kind of data with a transferable counterpart.
///
/// Implementors describe the form they take on when they have to leave the
/// thread that built them, and how to come back from it. Restoring is total:
/// anything that could go wrong was refused when the snapshot was taken.
pub trait Snapshot: Sized {
    /// The form that crosses the boundary.
    type Transferred: Send + Sync;

    /// Describes this value in a form another thread can receive.
    ///
    /// # Errors
    ///
    /// Returns [`SnapshotError::NotTransferable`] when the value holds a callable.
    fn snapshot(&self) -> Result<Self::Transferred, SnapshotError>;

    /// Rebuilds a value from what crossed the boundary.
    fn restore(transferred: Self::Transferred) -> Self;
}

/// A [`Value`] that is known not to hold a callable, and so can be sent.
///
/// Structurally [`Value`] without its function variant. Obtain one with
/// [`Snapshot::snapshot`] and read it back with [`Snapshot::restore`].
///
/// A restored [`Value`] belongs to the thread that restored it and cannot be
/// returned from one, so a worker that has something to report snapshots its own
/// result on the way back out.
///
/// ```
/// # use optimist::squiggle::{Runtime, Value, snapshot::Snapshot};
/// let mut runtime = Runtime::new();
/// let solved = runtime.evaluate("{ latency: 0.02, rate: normal(500, 20) }").expect("evaluates");
///
/// let sent = solved.snapshot()?;
/// let returned = std::thread::spawn(move || {
///     let value = Value::restore(sent);
///     let Value::Dictionary(entries) = &value else {
///         panic!("a dictionary crossed the boundary");
///     };
///     entries["rate"].snapshot()
/// })
/// .join()
/// .expect("no panic")?;
///
/// assert_eq!(Value::restore(returned).type_name(), "Distribution");
/// # Ok::<(), optimist::squiggle::SnapshotError>(())
/// ```
#[derive(Clone, Debug)]
pub enum Transferred {
    /// See [`Value::Number`].
    Number(f64),
    /// See [`Value::Boolean`].
    Boolean(bool),
    /// See [`Value::String`].
    String(String),
    /// See [`Value::Array`].
    Array(Vec<Transferred>),
    /// See [`Value::Dictionary`].
    Dictionary(BTreeMap<String, Transferred>),
    /// See [`Value::Distribution`]. Carries the draws by handle, not by copy.
    Distribution(Distribution),
    /// See [`Value::Date`].
    Date(DateValue),
    /// See [`Value::Duration`].
    Duration(DurationValue),
    /// See [`Value::Domain`].
    Domain(Domain),
    /// See [`Value::Void`].
    Void,
}

impl Snapshot for Value {
    type Transferred = Transferred;

    fn snapshot(&self) -> Result<Self::Transferred, SnapshotError> {
        Ok(match self {
            Self::Number(number) => Transferred::Number(*number),
            Self::Boolean(boolean) => Transferred::Boolean(*boolean),
            Self::String(string) => Transferred::String(string.clone()),
            Self::Distribution(distribution) => Transferred::Distribution(distribution.clone()),
            Self::Date(date) => Transferred::Date(*date),
            Self::Duration(duration) => Transferred::Duration(*duration),
            Self::Domain(domain) => Transferred::Domain(domain.clone()),
            Self::Void => Transferred::Void,
            Self::Array(items) => Transferred::Array(
                items
                    .iter()
                    .map(Value::snapshot)
                    .collect::<Result<_, _>>()?,
            ),
            Self::Dictionary(entries) => Transferred::Dictionary(
                entries
                    .iter()
                    .map(|(name, value)| Ok((name.clone(), value.snapshot()?)))
                    .collect::<Result<_, SnapshotError>>()?,
            ),
            Self::Function(_) => {
                return Err(SnapshotError::NotTransferable {
                    found: self.type_name(),
                });
            }
        })
    }

    fn restore(transferred: Self::Transferred) -> Self {
        match transferred {
            Transferred::Number(number) => Self::Number(number),
            Transferred::Boolean(boolean) => Self::Boolean(boolean),
            Transferred::String(string) => Self::String(string),
            Transferred::Distribution(distribution) => Self::Distribution(distribution),
            Transferred::Date(date) => Self::Date(date),
            Transferred::Duration(duration) => Self::Duration(duration),
            Transferred::Domain(domain) => Self::Domain(domain),
            Transferred::Void => Self::Void,
            Transferred::Array(items) => {
                Self::Array(items.into_iter().map(Value::restore).collect())
            }
            Transferred::Dictionary(entries) => Self::dictionary(
                entries
                    .into_iter()
                    .map(|(name, value)| (name, Value::restore(value)))
                    .collect(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of the type: the compiler agrees it can be sent.
    const fn _transferable_is_sendable<T: Send + Sync>() {}
    const _: () = _transferable_is_sendable::<Transferred>();

    fn sampled(draws: Vec<f64>) -> Value {
        Value::Distribution(Distribution::from_samples(draws).expect("samples"))
    }

    fn round_trip(value: &Value) -> Value {
        Value::restore(value.snapshot().expect("transferable"))
    }

    #[test]
    fn plain_values_survive_the_crossing() {
        for value in [
            Value::Number(2.5),
            Value::Boolean(true),
            Value::String("api".to_owned()),
            Value::Void,
        ] {
            assert_eq!(format!("{:?}", round_trip(&value)), format!("{value:?}"));
        }
    }

    #[test]
    fn nested_collections_survive_the_crossing() {
        let value = Value::dictionary(BTreeMap::from([
            ("rate".to_owned(), Value::Number(500.0)),
            (
                "stages".to_owned(),
                Value::Array(vec![Value::Number(1.0), Value::String("edge".to_owned())]),
            ),
        ]));
        assert_eq!(format!("{:?}", round_trip(&value)), format!("{value:?}"));
    }

    /// Two uses of one quantity must still be one quantity afterwards.
    ///
    /// This is what stops `x - x` from acquiring a spread once a solve has been
    /// handed between threads.
    #[test]
    fn two_uses_of_one_quantity_still_share_their_draws() {
        let quantity = sampled(vec![1.0, 2.0, 3.0]);
        let both = Value::Array(vec![quantity.clone(), quantity]);

        let Value::Array(restored) = round_trip(&both) else {
            panic!("an array restores as an array");
        };
        let [Value::Distribution(left), Value::Distribution(right)] = &restored[..] else {
            panic!("both entries restore as distributions");
        };
        assert!(
            std::ptr::eq(
                left.samples().expect("draws"),
                right.samples().expect("draws")
            ),
            "the two uses must read one sample set, not two copies of it"
        );
    }

    #[test]
    fn a_callable_is_refused_rather_than_dropped() {
        let mut runtime = crate::squiggle::Runtime::new();
        let function = runtime.evaluate("{|x| x + 1}").expect("evaluates");
        assert_eq!(
            function.snapshot().err(),
            Some(SnapshotError::NotTransferable {
                found: "Function"
            })
        );
    }

    /// A callable buried in a collection is found rather than carried along.
    #[test]
    fn a_callable_inside_a_collection_is_refused_too() {
        let mut runtime = crate::squiggle::Runtime::new();
        let function = runtime.evaluate("{|x| x}").expect("evaluates");
        let value = Value::dictionary(BTreeMap::from([("apply".to_owned(), function)]));
        assert!(value.snapshot().is_err());
    }
}
