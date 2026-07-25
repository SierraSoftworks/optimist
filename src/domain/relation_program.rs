use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::squiggle::{
    Runtime, RuntimeConfig, Value, ast::Program, builtin_names, lint_program, parse,
};

use super::{Unit, squiggle_estimate::squiggle_unit};

/// Largest relation source accepted, matching the estimate source limit.
const MAX_SOURCE_BYTES: usize = 65_536;
const MAX_STEPS: usize = 100_000;
const BINDINGS_MODULE: &str = "optimist/bindings";
const RESULT_BINDING: &str = "optimist_result";
const MODULE_BINDING: &str = "optimist_bindings";
const BASELINE_BINDING: &str = "baseline";

/// Declared names and units a relation may reference.
///
/// The schema is derived from the graph rather than authored: parents come from
/// incoming causal relationships, activations from intervention effects, and
/// parameters from the relation's own uncertain coefficients. Declaring them
/// up front is what lets the unit checker verify the author's arithmetic.
#[derive(Clone, Debug, PartialEq)]
pub struct RelationSchema {
    /// Canonical unit the relation must produce.
    pub result_unit: Unit,
    /// Native unit of each referenceable parent state, keyed by name.
    pub parents: BTreeMap<String, Unit>,
    /// Unit of each uncertain coefficient owned by the relation.
    pub parameters: BTreeMap<String, Unit>,
    /// Dimensionless intervention activations reaching this state.
    pub activations: BTreeSet<String>,
}

impl RelationSchema {
    /// Creates a schema for a relation producing `result_unit` with no inputs.
    pub fn new(result_unit: Unit) -> Self {
        Self {
            result_unit,
            parents: BTreeMap::new(),
            parameters: BTreeMap::new(),
            activations: BTreeSet::new(),
        }
    }
}

/// Sampled scalar values supplied for one relation evaluation.
///
/// Values are scalars rather than distributions because propagation already
/// resolves one sample per draw. Feeding distributions here would resample
/// shared assumptions independently and silently break common-cause structure.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RelationBindings {
    /// The owning state's own sampled baseline, for relative expressions.
    pub baseline: f64,
    /// Parent state values, already lagged by their relationship.
    pub parents: BTreeMap<String, f64>,
    /// Sampled coefficients, held constant across a draw.
    pub parameters: BTreeMap<String, f64>,
    /// Intervention activation in `[0, 1]` for the current period.
    pub activations: BTreeMap<String, f64>,
}

impl RelationBindings {
    fn module(&self) -> Value {
        Value::Dictionary(BTreeMap::from([
            (BASELINE_BINDING.to_owned(), Value::Number(self.baseline)),
            ("parents".to_owned(), numbers(&self.parents)),
            ("parameters".to_owned(), numbers(&self.parameters)),
            ("activations".to_owned(), numbers(&self.activations)),
        ]))
    }
}

fn numbers(values: &BTreeMap<String, f64>) -> Value {
    Value::Dictionary(
        values
            .iter()
            .map(|(name, value)| (name.clone(), Value::Number(*value)))
            .collect(),
    )
}

/// A parsed, unit-checked state relation ready for repeated evaluation.
///
/// Compiling separates the expensive work, parsing and static checking, from the
/// per-period work, so a projection pays for the syntax tree once and then
/// evaluates it for every draw and period.
#[derive(Clone, Debug)]
pub struct RelationProgram {
    program: Program,
}

impl RelationProgram {
    /// Parses and unit-checks `source` against the names its schema declares.
    pub fn compile(source: &str, schema: &RelationSchema) -> Result<Self, RelationError> {
        let source = source.trim();
        if source.is_empty() {
            return Err(RelationError::EmptySource);
        }
        if source.len() > MAX_SOURCE_BYTES {
            return Err(RelationError::SourceTooLarge);
        }
        let wrapped = wrapped_source(source, schema)?;
        let program = parse(&wrapped).map_err(|diagnostics| {
            RelationError::Diagnostic(diagnostics.first().map_or_else(
                || "invalid relation".to_owned(),
                |value| value.message.clone(),
            ))
        })?;
        if let Some(diagnostic) = lint_program(&program).into_iter().next() {
            return Err(RelationError::Diagnostic(diagnostic.message));
        }
        Ok(Self { program })
    }

    /// Creates a runtime this program can be evaluated on repeatedly.
    pub fn runtime(seed: u64) -> Result<Runtime, RelationError> {
        Runtime::with_config(RuntimeConfig {
            seed,
            sample_count: 1,
            max_steps: MAX_STEPS,
        })
        .map_err(RelationError::Diagnostic)
    }

    /// Evaluates the relation for one set of sampled bindings.
    ///
    /// A relation is a deterministic function of its bindings: every uncertain
    /// input is sampled before it arrives. Returning a distribution instead of a
    /// number therefore means uncertainty was authored in the wrong place, and is
    /// rejected rather than silently collapsed to a mean.
    pub fn evaluate(
        &self,
        runtime: &mut Runtime,
        bindings: &RelationBindings,
    ) -> Result<f64, RelationError> {
        runtime.register_module(BINDINGS_MODULE, bindings.module());
        let value = runtime
            .evaluate_program(&self.program)
            .map_err(|diagnostic| RelationError::Diagnostic(diagnostic.message))?;
        match value {
            Value::Number(value) if value.is_finite() => Ok(value),
            _ => Err(RelationError::NonNumericResult),
        }
    }
}

/// Builds the generated prelude which binds and unit-annotates every name.
fn wrapped_source(source: &str, schema: &RelationSchema) -> Result<String, RelationError> {
    let mut lines = vec![format!("import \"{BINDINGS_MODULE}\" as {MODULE_BINDING}")];
    lines.push(format!(
        "{BASELINE_BINDING} :: {} = {MODULE_BINDING}.{BASELINE_BINDING}",
        squiggle_unit(&schema.result_unit)
    ));
    let mut declared = BTreeSet::from([BASELINE_BINDING.to_owned()]);
    for (group, names) in [
        ("parents", &schema.parents),
        ("parameters", &schema.parameters),
    ] {
        for (name, unit) in names {
            declare(&mut declared, name)?;
            lines.push(format!(
                "{name} :: {} = {MODULE_BINDING}.{group}.{name}",
                squiggle_unit(unit)
            ));
        }
    }
    for name in &schema.activations {
        declare(&mut declared, name)?;
        lines.push(format!("{name} :: 1 = {MODULE_BINDING}.activations.{name}"));
    }
    lines.push(format!(
        "{RESULT_BINDING} :: {} = {{\n{source}\n}}\n{RESULT_BINDING}",
        squiggle_unit(&schema.result_unit)
    ));
    Ok(lines.join("\n"))
}

fn declare(declared: &mut BTreeSet<String>, name: &str) -> Result<(), RelationError> {
    if !is_identifier(name) {
        return Err(RelationError::InvalidBindingName(name.to_owned()));
    }
    if name == RESULT_BINDING
        || name == MODULE_BINDING
        || builtin_names().contains(&name)
        || !declared.insert(name.to_owned())
    {
        return Err(RelationError::ReservedBindingName(name.to_owned()));
    }
    Ok(())
}

fn is_identifier(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/// Failures which prevent a state relation from compiling or evaluating.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum RelationError {
    /// The relation body is empty after trimming.
    #[error("a relation requires a calculation")]
    EmptySource,
    /// The relation body exceeds the retained source limit.
    #[error("a relation may not exceed {MAX_SOURCE_BYTES} bytes")]
    SourceTooLarge,
    /// A bound name cannot be written as a Squiggle identifier.
    #[error("relation binding {0:?} is not a valid Squiggle identifier")]
    InvalidBindingName(String),
    /// A bound name collides with a builtin or a generated binding.
    #[error("relation binding {0:?} is reserved or declared twice")]
    ReservedBindingName(String),
    /// Parsing, unit checking, or evaluation reported a diagnostic.
    #[error("{0}")]
    Diagnostic(String),
    /// The relation produced something other than a finite number.
    #[error("a relation must produce a finite number; author uncertainty as a parameter instead")]
    NonNumericResult,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(terms: &[(&str, i32)]) -> Unit {
        Unit::from_exponents(terms.iter().map(|(name, exponent)| (*name, *exponent))).unwrap()
    }

    /// Customer impact as outage frequency multiplied by impact duration.
    fn product_schema() -> RelationSchema {
        let mut schema = RelationSchema::new(unit(&[("minute", 1), ("year", -1)]));
        schema.parents.insert(
            "outage_frequency".to_owned(),
            unit(&[("outage", 1), ("year", -1)]),
        );
        schema.parents.insert(
            "impact_duration".to_owned(),
            unit(&[("minute", 1), ("outage", -1)]),
        );
        schema
    }

    fn evaluate(program: &RelationProgram, bindings: &RelationBindings) -> f64 {
        let mut runtime = RelationProgram::runtime(42).unwrap();
        program.evaluate(&mut runtime, bindings).unwrap()
    }

    #[test]
    fn evaluates_a_product_of_two_parents_in_their_derived_unit() {
        let program =
            RelationProgram::compile("outage_frequency * impact_duration", &product_schema())
                .unwrap();
        let bindings = RelationBindings {
            baseline: 0.0,
            parents: BTreeMap::from([
                ("outage_frequency".to_owned(), 6.0),
                ("impact_duration".to_owned(), 90.0),
            ]),
            ..RelationBindings::default()
        };
        assert_eq!(evaluate(&program, &bindings), 540.0);
    }

    #[test]
    fn rejects_a_product_whose_unit_disagrees_with_the_state() {
        // outage/year times minute/outage is minute/year, never minute.
        let mut schema = product_schema();
        schema.result_unit = unit(&[("minute", 1)]);
        assert!(matches!(
            RelationProgram::compile("outage_frequency * impact_duration", &schema),
            Err(RelationError::Diagnostic(_))
        ));
    }

    #[test]
    fn rejects_addition_across_incompatible_units() {
        assert!(matches!(
            RelationProgram::compile("outage_frequency + impact_duration", &product_schema()),
            Err(RelationError::Diagnostic(_))
        ));
    }

    #[test]
    fn scales_a_baseline_by_an_intervention_activation() {
        let mut schema = RelationSchema::new(unit(&[("change", 1), ("month", -1)]));
        schema.activations.insert("code_yellow".to_owned());
        schema
            .parameters
            .insert("suppression".to_owned(), Unit::dimensionless());
        let program =
            RelationProgram::compile("baseline * (1 - suppression * code_yellow)", &schema)
                .unwrap();
        let bindings = RelationBindings {
            baseline: 200.0,
            parameters: BTreeMap::from([("suppression".to_owned(), 0.9)]),
            activations: BTreeMap::from([("code_yellow".to_owned(), 1.0)]),
            ..RelationBindings::default()
        };
        assert!((evaluate(&program, &bindings) - 20.0).abs() < 1e-12);

        let inactive = RelationBindings {
            activations: BTreeMap::from([("code_yellow".to_owned(), 0.0)]),
            ..bindings
        };
        assert_eq!(evaluate(&program, &inactive), 200.0);
    }

    #[test]
    fn rejects_an_undeclared_reference() {
        assert!(matches!(
            RelationProgram::compile("outage_frequency * missing", &product_schema()),
            Err(RelationError::Diagnostic(_))
        ));
    }

    #[test]
    fn rejects_uncertainty_authored_inside_the_relation() {
        let schema = RelationSchema::new(Unit::dimensionless());
        let program = RelationProgram::compile("normal(1, 0.1)", &schema).unwrap();
        let mut runtime = RelationProgram::runtime(42).unwrap();
        assert_eq!(
            program.evaluate(&mut runtime, &RelationBindings::default()),
            Err(RelationError::NonNumericResult)
        );
    }

    #[test]
    fn rejects_binding_names_which_collide_or_are_unwritable() {
        let mut reserved = RelationSchema::new(Unit::dimensionless());
        reserved
            .parameters
            .insert("baseline".to_owned(), Unit::dimensionless());
        assert_eq!(
            RelationProgram::compile("baseline", &reserved).err(),
            Some(RelationError::ReservedBindingName("baseline".to_owned()))
        );

        let mut builtin = RelationSchema::new(Unit::dimensionless());
        builtin
            .parameters
            .insert("normal".to_owned(), Unit::dimensionless());
        assert_eq!(
            RelationProgram::compile("normal", &builtin).err(),
            Some(RelationError::ReservedBindingName("normal".to_owned()))
        );

        let mut invalid = RelationSchema::new(Unit::dimensionless());
        invalid
            .parameters
            .insert("2rate".to_owned(), Unit::dimensionless());
        assert_eq!(
            RelationProgram::compile("1", &invalid).err(),
            Some(RelationError::InvalidBindingName("2rate".to_owned()))
        );
    }

    #[test]
    fn rejects_empty_and_oversized_sources() {
        let schema = RelationSchema::new(Unit::dimensionless());
        assert_eq!(
            RelationProgram::compile("   ", &schema).err(),
            Some(RelationError::EmptySource)
        );
        assert_eq!(
            RelationProgram::compile(&"1 + ".repeat(20_000), &schema).err(),
            Some(RelationError::SourceTooLarge)
        );
    }

    #[test]
    fn reuses_one_runtime_across_many_evaluations_deterministically() {
        let program =
            RelationProgram::compile("outage_frequency * impact_duration", &product_schema())
                .unwrap();
        let mut runtime = RelationProgram::runtime(7).unwrap();
        for period in 1..=200_u32 {
            let bindings = RelationBindings {
                parents: BTreeMap::from([
                    ("outage_frequency".to_owned(), f64::from(period)),
                    ("impact_duration".to_owned(), 2.0),
                ]),
                ..RelationBindings::default()
            };
            assert_eq!(
                program.evaluate(&mut runtime, &bindings).unwrap(),
                f64::from(period) * 2.0,
                "rebinding must not leak state between evaluations"
            );
        }
    }
}
