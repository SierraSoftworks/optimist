//! Turning modelled quantities into figures a reader can scan.
//!
//! Two decisions here carry most of the weight. Numbers are shown to four
//! significant figures and switched to exponent form once they leave the range
//! a person reads at a glance, because a headroom of `-3004303674979.1333` in a
//! column tells nobody anything a `-3.004e12` does not. And an uncertain
//! quantity is reduced to a mean beside a central eighty percent interval,
//! because a single number would hide the spread the whole model exists to
//! carry and the full distribution does not fit in a column.

use colored::{ColoredString, Colorize};

use crate::squiggle::Value;

/// How a figure should read at a glance.
///
/// Colour is the fastest signal a terminal has, so it is spent on the one
/// question a report is asked: is this within its limit, near it, or past it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Tone {
    /// Ordinary text, left to the terminal's own foreground colour.
    #[default]
    Plain,
    /// An identifier, brightened so a column of them is easy to follow.
    Name,
    /// Comfortably within a limit.
    Good,
    /// Close enough to a limit to be worth watching.
    Warn,
    /// Past a limit, or otherwise a finding somebody has to act on.
    Bad,
    /// Context that should not compete with the numbers beside it.
    Muted,
}

impl Tone {
    /// Applies this tone to `text`.
    pub(crate) fn paint(self, text: &str) -> ColoredString {
        match self {
            Self::Plain => text.normal(),
            Self::Name => text.bright_white(),
            Self::Good => text.green(),
            Self::Warn => text.yellow(),
            Self::Bad => text.red(),
            Self::Muted => text.bright_black(),
        }
    }

    /// Chooses a tone for a demand expressed as a share of the limit it consumes.
    ///
    /// Eight tenths is the warning point because a constraint that close to its
    /// limit on average is already over it in a good share of draws.
    pub(crate) fn for_utilisation(utilisation: f64) -> Self {
        if !utilisation.is_finite() || utilisation >= 1.0 {
            Self::Bad
        } else if utilisation >= 0.8 {
            Self::Warn
        } else {
            Self::Good
        }
    }

    /// Chooses a tone for a probability of binding.
    ///
    /// Any exposure at all is worth noticing, so this is harsher than a linear
    /// reading of the number would be.
    pub(crate) fn for_probability(probability: f64) -> Self {
        if probability >= 0.5 {
            Self::Bad
        } else if probability > 0.0 {
            Self::Warn
        } else {
            Self::Good
        }
    }
}

/// Renders a number in the narrowest form that keeps it readable.
pub(crate) fn number(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_owned();
    }
    if value.is_infinite() {
        return if value.is_sign_positive() {
            "∞"
        } else {
            "-∞"
        }
        .to_owned();
    }

    let magnitude = value.abs();
    if magnitude != 0.0 && !(1e-3..1e6).contains(&magnitude) {
        return format!("{value:.3e}");
    }

    let text = format!("{value:.4}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Renders a probability as a whole percentage.
pub(crate) fn percentage(probability: f64) -> String {
    format!("{:.0}%", probability * 100.0)
}

/// Counts something, in words that agree with the number.
pub(crate) fn plural(quantity: usize, one: &str, many: &str) -> String {
    if quantity == 1 {
        format!("{quantity} {one}")
    } else {
        format!("{quantity} {many}")
    }
}

/// Renders a demand expressed as a share of the limit it consumes.
///
/// Utilisation is read for the side of one it falls on, not for its fourth
/// decimal place, so anything above one is rounded hard and the precision is
/// spent where the difference between 0.06 and 0.006 still matters.
pub(crate) fn ratio(utilisation: f64) -> String {
    if !utilisation.is_finite() {
        return number(utilisation);
    }
    match utilisation.abs() {
        magnitude if magnitude >= 100.0 => format!("{utilisation:.0}"),
        magnitude if magnitude >= 1.0 => format!("{utilisation:.2}"),
        _ => number(utilisation),
    }
}

/// Splits a solved quantity into the figure to show and the spread around it.
///
/// The second element is empty for a quantity that carries no uncertainty,
/// which keeps a column of certain values from being padded out by a range
/// that says nothing.
pub(crate) fn quantity(value: &Value) -> (String, String) {
    match value {
        Value::Number(figure) => (number(*figure), String::new()),
        Value::Distribution(distribution) => {
            let (Ok(mean), Ok(low), Ok(high)) = (
                distribution.mean(),
                distribution.quantile(0.1),
                distribution.quantile(0.9),
            ) else {
                return ("unavailable".to_owned(), String::new());
            };
            (number(mean), format!("{} .. {}", number(low), number(high)))
        }
        other => (other.type_name().to_owned(), String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_stay_readable_at_every_magnitude() {
        assert_eq!(number(685.155), "685.155");
        assert_eq!(number(8.0), "8");
        assert_eq!(number(0.0), "0");
        assert_eq!(number(-3004303674979.1333), "-3.004e12");
        assert_eq!(number(0.00002), "2.000e-5");
        assert_eq!(number(f64::INFINITY), "∞");
    }

    #[test]
    fn tones_follow_how_close_a_constraint_is_to_binding() {
        assert_eq!(Tone::for_utilisation(0.2), Tone::Good);
        assert_eq!(Tone::for_utilisation(0.85), Tone::Warn);
        assert_eq!(Tone::for_utilisation(1.4), Tone::Bad);
        assert_eq!(Tone::for_probability(0.0), Tone::Good);
        assert_eq!(Tone::for_probability(0.01), Tone::Warn);
        assert_eq!(Tone::for_probability(0.9), Tone::Bad);
    }

    #[test]
    fn a_certain_quantity_carries_no_interval() {
        assert_eq!(
            quantity(&Value::Number(4.0)),
            ("4".to_owned(), String::new())
        );
    }

    #[test]
    fn utilisation_is_rounded_hardest_where_it_matters_least() {
        assert_eq!(ratio(109.8648), "110");
        assert_eq!(ratio(7.0086), "7.01");
        assert_eq!(ratio(0.0068), "0.0068");
    }
}
