//! Evaluating one expression the way the solver would.
//!
//! An author writing `900 * lognormal(0, 0.4)` cannot see what they have
//! written until something solves the design around it, by which time the
//! expression is one of a dozen inputs to a number several components away.
//! Showing the quantity itself, while it is being typed, is the difference
//! between writing uncertainty and guessing at it.
//!
//! The evaluation is the solver's own: the same runtime, the same seed
//! derivation, and the same visible scope. A preview produced by a second, more
//! forgiving evaluator would be worse than none, because it would disagree with
//! the answer at exactly the moments an author most needs to trust it.

use std::collections::BTreeMap;

use crate::squiggle::Value;

use super::{
    compile::{Timing, quantities, runtime, syntax},
    evaluate::{EvaluationConfig, EvaluationError},
    model::SystemModel,
};

/// Evaluates `expression` against the shared quantities visible to it.
///
/// `before` names the entry being edited. Shared quantities are evaluated in
/// declaration order and can see only the ones ahead of them, so a preview of
/// the third entry must not be shown the fourth — and an entry that referred to
/// itself would recurse rather than resolve. Passing `None` evaluates against
/// the whole scratchpad, which is what a quantity being added sees.
///
/// ```
/// use optimist::system::{EvaluationConfig, SystemModel, preview};
///
/// let model: SystemModel = serde_yaml_ng::from_str("
/// scratchpad:
///   - name: peak_rate
///     expression: '900'
/// ")?;
///
/// let value = preview(&model, "peak_rate * 2", None, EvaluationConfig::default())?;
/// assert_eq!(value.as_number(), Some(1800.0));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`EvaluationError::Syntax`] where the expression does not parse and
/// [`EvaluationError::Evaluation`] where it parses but cannot be evaluated —
/// which includes referring to a quantity declared after it.
pub fn preview(
    model: &SystemModel,
    expression: &str,
    before: Option<&str>,
    config: EvaluationConfig,
) -> Result<Value, EvaluationError> {
    let timing = Timing {
        seed: config.seed,
        ensemble: config.ensemble(),
        time: 0.0,
        step: config.step,
    };
    let visible = before
        .and_then(|name| model.scratchpad.iter().position(|entry| entry.name == name))
        .unwrap_or(model.scratchpad.len());
    let globals = quantities(&model.scratchpad[..visible], &BTreeMap::new(), timing)?;

    let program = syntax(expression).map_err(|diagnostics| EvaluationError::Syntax {
        location: "expression".to_owned(),
        message: diagnostics.first().map_or_else(
            || "invalid expression".to_owned(),
            |first| first.message.clone(),
        ),
    })?;
    runtime(config.seed, config.ensemble())?
        .evaluate_values(
            &program,
            globals
                .iter()
                .map(|(name, value)| (name.as_str(), value.clone()))
                .chain(timing.clock()),
        )
        .map_err(|diagnostic| EvaluationError::Evaluation {
            location: "expression".to_owned(),
            message: diagnostic.message,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(source: &str) -> SystemModel {
        serde_yaml_ng::from_str(source).expect("model")
    }

    #[test]
    fn a_constant_previews_as_itself() {
        let value = preview(
            &model("scratchpad: []\n"),
            "42",
            None,
            EvaluationConfig::default(),
        )
        .expect("evaluates");
        assert_eq!(value.as_number(), Some(42.0));
    }

    #[test]
    fn an_expression_sees_the_quantities_declared_before_it() {
        let design =
            model("scratchpad:\n- name: a\n  expression: '2'\n- name: b\n  expression: '3'\n");
        let value =
            preview(&design, "a * b", None, EvaluationConfig::default()).expect("evaluates");
        assert_eq!(value.as_number(), Some(6.0));
    }

    /// An entry cannot see itself or anything after it, so nor can its preview.
    ///
    /// Showing an author a figure the solver will refuse would be worse than
    /// showing them the refusal.
    #[test]
    fn an_expression_cannot_see_itself_or_what_follows() {
        let design =
            model("scratchpad:\n- name: a\n  expression: '2'\n- name: b\n  expression: 'a * 3'\n");
        assert!(preview(&design, "b + 1", Some("b"), EvaluationConfig::default()).is_err());
        assert_eq!(
            preview(&design, "a + 1", Some("b"), EvaluationConfig::default())
                .expect("evaluates")
                .as_number(),
            Some(3.0),
        );
    }

    #[test]
    fn an_uncertain_expression_previews_as_a_spread() {
        let value = preview(
            &model("scratchpad: []\n"),
            "lognormal(0, 0.4)",
            None,
            EvaluationConfig {
                sample_count: 500,
                ..EvaluationConfig::default()
            },
        )
        .expect("evaluates");
        let distribution = value.as_distribution().expect("a distribution");
        assert!(
            distribution.quantile(0.1).expect("p10") < distribution.quantile(0.9).expect("p90")
        );
    }

    #[test]
    fn a_broken_expression_says_so() {
        let error = preview(
            &model("scratchpad: []\n"),
            "1 +",
            None,
            EvaluationConfig::default(),
        )
        .expect_err("refuses");
        assert!(matches!(error, EvaluationError::Syntax { .. }));
    }
}
