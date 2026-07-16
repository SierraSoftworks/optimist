use std::collections::BTreeMap;

use super::UnitError;

pub(super) fn from_exponents<I, S>(exponents: I) -> Result<BTreeMap<String, i32>, UnitError>
where
    I: IntoIterator<Item = (S, i32)>,
    S: Into<String>,
{
    let mut terms = BTreeMap::new();
    for (name, exponent) in exponents {
        let name = name.into();
        validate_name(&name)?;
        update_exponent(&mut terms, &name, exponent)?;
    }
    Ok(terms)
}

pub(super) fn multiply(
    left: &BTreeMap<String, i32>,
    right: &BTreeMap<String, i32>,
) -> Result<BTreeMap<String, i32>, UnitError> {
    let mut terms = left.clone();
    for (name, exponent) in right {
        update_exponent(&mut terms, name, *exponent)?;
    }
    Ok(terms)
}

pub(super) fn divide(
    numerator: &BTreeMap<String, i32>,
    denominator: &BTreeMap<String, i32>,
) -> Result<BTreeMap<String, i32>, UnitError> {
    let mut terms = numerator.clone();
    for (name, exponent) in denominator {
        let current = terms.get(name).copied().unwrap_or(0);
        let difference = current
            .checked_sub(*exponent)
            .ok_or(UnitError::ExponentOverflow)?;
        set_exponent(&mut terms, name, difference);
    }
    Ok(terms)
}

pub(super) fn power(
    source: &BTreeMap<String, i32>,
    power: i32,
) -> Result<BTreeMap<String, i32>, UnitError> {
    source
        .iter()
        .filter_map(|(name, exponent)| {
            let result = exponent.checked_mul(power);
            match result {
                Some(0) => None,
                Some(value) => Some(Ok((name.clone(), value))),
                None => Some(Err(UnitError::ExponentOverflow)),
            }
        })
        .collect()
}

pub(super) fn validate_name(name: &str) -> Result<(), UnitError> {
    let mut bytes = name.bytes();
    if name.len() > 64 || !bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic()) {
        return Err(UnitError::InvalidName(name.to_owned()));
    }
    if !bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-')) {
        return Err(UnitError::InvalidName(name.to_owned()));
    }
    Ok(())
}

fn update_exponent(
    terms: &mut BTreeMap<String, i32>,
    name: &str,
    delta: i32,
) -> Result<(), UnitError> {
    let exponent = terms
        .get(name)
        .copied()
        .unwrap_or(0)
        .checked_add(delta)
        .ok_or(UnitError::ExponentOverflow)?;
    set_exponent(terms, name, exponent);
    Ok(())
}

fn set_exponent(terms: &mut BTreeMap<String, i32>, name: &str, exponent: i32) {
    if exponent == 0 {
        terms.remove(name);
    } else {
        terms.insert(name.to_owned(), exponent);
    }
}
