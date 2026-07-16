use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

use super::unit_ops;

/// Failures produced while constructing or combining runtime units.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum UnitError {
    /// A base unit name is empty or outside the stable identifier grammar.
    #[error("invalid base unit name {0:?}")]
    InvalidName(String),
    /// A serialized dimension explicitly contains a zero exponent.
    #[error("base unit {0:?} has a non-canonical zero exponent")]
    ZeroExponent(String),
    /// Combining or scaling dimensions exceeded the supported integer range.
    #[error("unit exponent overflow")]
    ExponentOverflow,
}

/// A runtime physical or semantic dimension expressed as integer powers of base units.
///
/// Terms are stored in lexical order and zero powers are omitted, giving equivalent
/// products one canonical serialized representation. Optimist currently treats units
/// with the same terms as equal and does not model conversion scales or offsets; for
/// example, `m` and `km` are distinct custom base units until a registry is introduced.
/// Multiplication adds exponents, division subtracts them, and integer powers multiply
/// them. Every operation is checked for `i32` overflow.
///
/// ```
/// use optimist::domain::Unit;
///
/// let distance = Unit::base("m")?;
/// let time = Unit::base("s")?;
/// let speed = distance.checked_divide(&time)?;
/// assert_eq!(speed.exponent("m"), 1);
/// assert_eq!(speed.exponent("s"), -1);
/// # Ok::<(), optimist::domain::UnitError>(())
/// ```
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Unit(pub(super) BTreeMap<String, i32>);

/// The dimensional meaning of a runtime [`Unit`].
///
/// Conversion scales are not implemented, so a unit and its dimension currently
/// share the same canonical representation.
pub type Dimension = Unit;

impl Unit {
    /// Returns the multiplicative identity, which has no base-unit terms.
    pub fn dimensionless() -> Self {
        Self(BTreeMap::new())
    }

    /// Constructs a custom base unit with exponent one.
    ///
    /// Names must start with an ASCII letter, contain only ASCII letters, digits,
    /// `_`, `.`, or `-`, and be at most 64 bytes long.
    pub fn base(name: impl Into<String>) -> Result<Self, UnitError> {
        Self::from_exponents([(name.into(), 1)])
    }

    /// Constructs a canonical unit from base-unit names and integer exponents.
    ///
    /// Repeated names are combined with checked addition and resulting zero powers
    /// are omitted. This makes caller ordering irrelevant.
    pub fn from_exponents<I, S>(exponents: I) -> Result<Self, UnitError>
    where
        I: IntoIterator<Item = (S, i32)>,
        S: Into<String>,
    {
        unit_ops::from_exponents(exponents).map(Self)
    }

    /// Reports whether this unit has no dimensional terms.
    pub fn is_dimensionless(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the exponent for `name`, or zero when the base unit is absent.
    pub fn exponent(&self, name: &str) -> i32 {
        self.0.get(name).copied().unwrap_or(0)
    }

    /// Iterates over canonical `(base unit, exponent)` terms in lexical order.
    pub fn terms(&self) -> impl Iterator<Item = (&str, i32)> {
        self.0
            .iter()
            .map(|(name, exponent)| (name.as_str(), *exponent))
    }

    /// Multiplies two units by adding corresponding exponents.
    pub fn checked_multiply(&self, other: &Self) -> Result<Self, UnitError> {
        unit_ops::multiply(&self.0, &other.0).map(Self)
    }

    /// Divides two units by subtracting denominator exponents.
    pub fn checked_divide(&self, denominator: &Self) -> Result<Self, UnitError> {
        unit_ops::divide(&self.0, &denominator.0).map(Self)
    }

    /// Raises a unit to an integer power by multiplying every exponent.
    pub fn checked_power(&self, power: i32) -> Result<Self, UnitError> {
        unit_ops::power(&self.0, power).map(Self)
    }
}

impl<'de> Deserialize<'de> for Unit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let terms = BTreeMap::<String, i32>::deserialize(deserializer)?;
        for (name, exponent) in &terms {
            unit_ops::validate_name(name).map_err(de::Error::custom)?;
            if *exponent == 0 {
                return Err(de::Error::custom(UnitError::ZeroExponent(name.clone())));
            }
        }
        Ok(Self(terms))
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{Unit, UnitError};

    fn unit(meters: i32, seconds: i32) -> Unit {
        Unit::from_exponents([("m", meters), ("s", seconds)]).expect("small exponents")
    }

    #[test]
    fn rejects_invalid_names_and_non_canonical_json() {
        assert!(matches!(Unit::base(""), Err(UnitError::InvalidName(_))));
        assert!(matches!(Unit::base("1m"), Err(UnitError::InvalidName(_))));
        assert!(matches!(Unit::base("m/s"), Err(UnitError::InvalidName(_))));
        assert!(serde_json::from_str::<Unit>(r#"{"m":0}"#).is_err());
    }

    #[test]
    fn reports_exponent_overflow() {
        let maximum = Unit::from_exponents([("m", i32::MAX)]).expect("valid maximum");
        assert_eq!(
            maximum.checked_multiply(&Unit::base("m").unwrap()),
            Err(UnitError::ExponentOverflow)
        );
        let minimum = Unit::from_exponents([("s", i32::MIN)]).expect("valid minimum");
        assert_eq!(
            Unit::dimensionless().checked_divide(&minimum),
            Err(UnitError::ExponentOverflow)
        );
    }

    #[test]
    fn serde_uses_canonical_term_order() {
        let value = Unit::from_exponents([("s", -1), ("m", 1)]).expect("valid unit");
        assert_eq!(serde_json::to_string(&value).unwrap(), r#"{"m":1,"s":-1}"#);
        assert_eq!(
            serde_json::from_str::<Unit>(r#"{"s":-1,"m":1}"#).unwrap(),
            value
        );
    }

    proptest! {
        #[test]
        fn multiplication_is_commutative_and_canonical(
            left_m in -100_i32..100,
            left_s in -100_i32..100,
            right_m in -100_i32..100,
            right_s in -100_i32..100,
        ) {
            let left = unit(left_m, left_s);
            let right = unit(right_m, right_s);
            let left_right = left.checked_multiply(&right).unwrap();
            let right_left = right.checked_multiply(&left).unwrap();
            prop_assert_eq!(&left_right, &right_left);
            prop_assert_eq!(serde_json::to_string(&left_right).unwrap(), serde_json::to_string(&right_left).unwrap());
        }

        #[test]
        fn division_inverts_multiplication(
            left_m in -100_i32..100,
            left_s in -100_i32..100,
            right_m in -100_i32..100,
            right_s in -100_i32..100,
        ) {
            let left = unit(left_m, left_s);
            let right = unit(right_m, right_s);
            let product = left.checked_multiply(&right).unwrap();
            prop_assert_eq!(product.checked_divide(&right).unwrap(), left);
        }

        #[test]
        fn powers_multiply_exponents(meters in -100_i32..100, seconds in -100_i32..100, power in -10_i32..10) {
            let powered = unit(meters, seconds).checked_power(power).unwrap();
            prop_assert_eq!(powered.exponent("m"), meters * power);
            prop_assert_eq!(powered.exponent("s"), seconds * power);
            prop_assert_eq!(powered.is_dimensionless(), power == 0 || (meters == 0 && seconds == 0));
        }
    }
}
