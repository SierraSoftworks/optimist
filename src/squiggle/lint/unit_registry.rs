//! Canonical vocabulary for the `::` unit annotations the linter checks.
//!
//! # Canonicalisation, not conversion
//!
//! Annotations are checked statically and ignored at runtime, so the linter
//! never rescales a value. Two units are therefore interchangeable only when
//! they denote the same quantity at the same scale. `sec` and `seconds` fold to
//! `s` because they name one unit, while `ms` stays distinct from `s` because
//! adding them without conversion is an arithmetic error the author must fix
//! rather than a notation the checker should paper over.
//!
//! Compound names expand into their factors, so `rps` becomes `op * s^-1` and
//! compares equal to an explicitly written `op/s`. Expansion is what lets the
//! vocabulary stay small while the algebra in [`super::types::Unit`] does the
//! work of combining exponents.
//!
//! # Semantic base quantities
//!
//! Beyond time, information, and currency, three countable quantities are
//! tracked separately: `op`, `success`, and `error`. Keeping outcomes distinct
//! from demand is what makes reliability arithmetic checkable. An error ratio is
//! `error/op`, a service level indicator is `success/op`, and multiplying demand
//! by either yields an outcome rate:
//!
//! $$\text{op}\,\text{s}^{-1} \times \text{error}\,\text{op}^{-1} = \text{error}\,\text{s}^{-1}$$
//!
//! The cost is that `successes + errors` does not check as `op`, because a sum
//! of two distinct quantities has no common unit. That boundary is deliberate:
//! partitioning demand into outcomes is a modelling claim, so it is written
//! once and explicitly rather than assumed everywhere.
//!
//! Unrecognised names are preserved verbatim rather than rejected, so a model
//! may introduce its own quantity — `shard`, `tenant`, `widget` — and still have
//! it checked consistently against itself.

/// Expands one annotation factor into canonical `(unit, exponent)` terms.
pub(super) fn canonicalise(name: &str) -> Vec<(String, f64)> {
    if let Some(factors) = COMPOUND.iter().find(|(alias, _)| *alias == name) {
        return factors
            .1
            .iter()
            .map(|(unit, exponent)| ((*unit).to_owned(), *exponent))
            .collect();
    }
    let canonical = ALIASES
        .iter()
        .find(|(alias, _)| *alias == name)
        .map_or(name, |(_, canonical)| *canonical);
    vec![(canonical.to_owned(), 1.0)]
}

/// Spellings that name an existing unit rather than a new one.
const ALIASES: &[(&str, &str)] = &[
    ("second", "s"),
    ("seconds", "s"),
    ("sec", "s"),
    ("secs", "s"),
    ("millisecond", "ms"),
    ("milliseconds", "ms"),
    ("msec", "ms"),
    ("microsecond", "us"),
    ("microseconds", "us"),
    ("µs", "us"),
    ("nanosecond", "ns"),
    ("nanoseconds", "ns"),
    ("minute", "min"),
    ("minutes", "min"),
    ("hour", "h"),
    ("hours", "h"),
    ("hr", "h"),
    ("day", "d"),
    ("days", "d"),
    ("week", "wk"),
    ("weeks", "wk"),
    ("year", "y"),
    ("years", "y"),
    ("byte", "B"),
    ("bytes", "B"),
    ("bits", "bit"),
    ("operation", "op"),
    ("operations", "op"),
    ("ops", "op"),
    ("request", "op"),
    ("requests", "op"),
    ("req", "op"),
    ("reqs", "op"),
    ("query", "op"),
    ("queries", "op"),
    ("record", "op"),
    ("records", "op"),
    ("message", "op"),
    ("messages", "op"),
    ("successes", "success"),
    ("errors", "error"),
    ("failure", "error"),
    ("failures", "error"),
    ("usd", "USD"),
    ("dollar", "USD"),
    ("dollars", "USD"),
];

/// Names that stand for a product or ratio of other units.
const COMPOUND: &[(&str, &[(&str, f64)])] = &[
    ("rps", &[("op", 1.0), ("s", -1.0)]),
    ("qps", &[("op", 1.0), ("s", -1.0)]),
    ("iops", &[("op", 1.0), ("s", -1.0)]),
    ("hz", &[("s", -1.0)]),
    ("Bps", &[("B", 1.0), ("s", -1.0)]),
    ("bps", &[("bit", 1.0), ("s", -1.0)]),
    ("sli", &[("success", 1.0), ("op", -1.0)]),
    ("slo", &[("success", 1.0), ("op", -1.0)]),
    ("availability", &[("success", 1.0), ("op", -1.0)]),
    ("errorRatio", &[("error", 1.0), ("op", -1.0)]),
    ("errorBudget", &[("error", 1.0), ("op", -1.0)]),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn units(name: &str) -> Vec<(String, f64)> {
        canonicalise(name)
    }

    #[test]
    fn spellings_of_one_unit_agree() {
        assert_eq!(units("seconds"), units("sec"));
        assert_eq!(units("seconds"), units("s"));
        assert_eq!(units("requests"), units("op"));
    }

    #[test]
    fn scale_distinct_units_stay_distinct() {
        assert_ne!(units("ms"), units("s"));
        assert_ne!(units("KiB"), units("B"));
    }

    #[test]
    fn outcomes_stay_distinct_from_demand() {
        assert_ne!(units("success"), units("op"));
        assert_ne!(units("error"), units("op"));
        assert_ne!(units("success"), units("error"));
    }

    #[test]
    fn compound_names_expand_to_their_factors() {
        assert_eq!(
            units("rps"),
            vec![("op".to_owned(), 1.0), ("s".to_owned(), -1.0)]
        );
        assert_eq!(units("iops"), units("qps"));
        assert_eq!(units("hz"), vec![("s".to_owned(), -1.0)]);
    }

    #[test]
    fn service_levels_are_outcomes_per_operation() {
        assert_eq!(
            units("sli"),
            vec![("success".to_owned(), 1.0), ("op".to_owned(), -1.0)]
        );
        assert_eq!(units("availability"), units("slo"));
        assert_eq!(units("errorBudget"), units("errorRatio"));
    }

    #[test]
    fn unrecognised_names_are_preserved() {
        assert_eq!(units("shard"), vec![("shard".to_owned(), 1.0)]);
    }
}
