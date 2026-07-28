//! Deterministic Squiggle program evaluation.
//!
//! Distribution algebra draws independent operands for each Monte Carlo sample.
//! The default 10,000 draws are an approximation with standard error decreasing
//! as $O(n^{-1/2})$; callers should increase the count for tail-sensitive work.
//! A run resets its ChaCha20 stream to the configured seed, so identical source,
//! modules, and configuration replay exactly. The step limit bounds recursion and
//! expensive user functions, but does not impose wall-clock timing.

use std::collections::BTreeMap;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

use crate::profile::count;

use super::{Diagnostic, Value, ast::Program, parse, value::Environment};

mod builtin;
#[macro_use]
mod builtin_macros;
mod builtin_collection;
mod builtin_common;
mod builtin_core;
mod builtin_dict_extra;
mod builtin_distribution;
mod builtin_domain;
mod builtin_list_extra;
mod builtin_queueing;
mod builtin_reliability;
mod builtin_sampleset;
mod builtin_scoring;
mod builtin_temporal;
mod elementwise;
mod evaluator;
mod operation;
mod standard;

/// Deterministic evaluation and Monte Carlo limits for one runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    /// Seed used to reset the ChaCha20 stream before each run.
    pub seed: u64,
    /// Draws used when distribution algebra requires Monte Carlo propagation.
    pub sample_count: usize,
    /// Maximum AST evaluations and function calls in one run.
    pub max_steps: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            seed: 0,
            sample_count: 10_000,
            max_steps: 1_000_000,
        }
    }
}

/// A reusable, isolated Squiggle evaluator with explicitly registered modules.
pub struct Runtime {
    pub(super) config: RuntimeConfig,
    pub(super) rng: ChaCha20Rng,
    pub(super) steps: usize,
    pub(super) modules: BTreeMap<String, Value>,
    /// Standard globals, built once and shared by every run as a parent scope.
    pub(super) globals: Environment,
    /// Scope holding the values passed to [`Runtime::evaluate_values`], reused so
    /// that repeatedly binding the same names costs only the values.
    bindings: Environment,
}

/// The value and named exports produced by evaluating a Squiggle module.
#[derive(Clone, Debug, PartialEq)]
pub struct ModuleOutput {
    /// Optional final expression, represented as [`Value::Void`] when absent.
    pub value: Value,
    /// Bindings explicitly declared with `export`.
    pub exports: BTreeMap<String, Value>,
}

impl Runtime {
    /// Creates a runtime with deterministic defaults.
    pub fn new() -> Self {
        let config = RuntimeConfig::default();
        let globals = standard::environment();
        Self {
            config,
            rng: ChaCha20Rng::seed_from_u64(config.seed),
            steps: 0,
            modules: BTreeMap::new(),
            bindings: globals.child(),
            globals,
        }
    }

    /// Creates a runtime with validated resource and sampling limits.
    pub fn with_config(config: RuntimeConfig) -> Result<Self, String> {
        if config.sample_count == 0 {
            return Err("sample_count must be greater than zero".into());
        }
        if config.max_steps == 0 {
            return Err("max_steps must be greater than zero".into());
        }
        let globals = standard::environment();
        Ok(Self {
            config,
            rng: ChaCha20Rng::seed_from_u64(config.seed),
            steps: 0,
            modules: BTreeMap::new(),
            bindings: globals.child(),
            globals,
        })
    }

    /// Returns this runtime's immutable configuration.
    pub fn config(&self) -> RuntimeConfig {
        self.config
    }

    /// Registers a value that `import path as name` can bind during evaluation.
    pub fn register_module(&mut self, path: impl Into<String>, value: Value) {
        self.modules.insert(path.into(), value);
    }

    /// Parses and evaluates a complete Squiggle module.
    ///
    /// ```
    /// let mut runtime = optimist::squiggle::Runtime::new();
    /// let value = runtime.evaluate("square(x) = x^2\nsquare(4)")?;
    /// assert_eq!(value.as_number(), Some(16.0));
    /// # Ok::<(), Vec<optimist::squiggle::Diagnostic>>(())
    /// ```
    pub fn evaluate(&mut self, source: &str) -> Result<Value, Vec<Diagnostic>> {
        let program = parse(source)?;
        self.evaluate_program(&program).map_err(|error| vec![error])
    }

    /// Evaluates an already parsed module using this runtime's modules and limits.
    pub fn evaluate_program(&mut self, program: &Program) -> Result<Value, Diagnostic> {
        self.evaluate_program_output(program)
            .map(|output| output.value)
    }

    /// Parses and evaluates a module while retaining its explicit exports.
    pub fn evaluate_module(&mut self, source: &str) -> Result<ModuleOutput, Vec<Diagnostic>> {
        let program = parse(source)?;
        self.evaluate_program_output(&program)
            .map_err(|error| vec![error])
    }

    /// Evaluates a parsed module while retaining its explicit exports.
    ///
    /// Each run evaluates in a child of this runtime's standard globals, so
    /// imports and top-level bindings are discarded afterwards while the ~100
    /// builtin entries are built once rather than per evaluation. That matters
    /// when a caller evaluates the same program for every period of every draw.
    pub fn evaluate_program_output(
        &mut self,
        program: &Program,
    ) -> Result<ModuleOutput, Diagnostic> {
        self.steps = 0;
        self.rng = ChaCha20Rng::seed_from_u64(self.config.seed);
        let environment = self.globals.child();
        self.eval_program(program, &environment)
    }

    /// Evaluates a program against named values supplied by the caller.
    ///
    /// Registering a module and importing it copies the whole exported value into
    /// the run's scope, which is the wrong shape for a caller that re-evaluates
    /// one small program with different numbers thousands of times. Defining the
    /// values directly costs one scope entry each instead.
    ///
    /// Bindings may carry whole distributions and dictionaries, which is what
    /// lets a caller pass an already-sampled quantity in rather than having the
    /// program construct it. The random stream restarts on every call, so
    /// re-evaluating one program against the same bindings returns the same
    /// answer. A caller iterating toward a fixed point depends on that: draws
    /// that shifted between passes would leave the iteration chasing sampling
    /// noise it could never converge against.
    ///
    /// The bindings live in a scope this runtime keeps between calls, so binding
    /// a name it has already seen writes a value over an existing key rather than
    /// allocating a fresh one. Names left behind by an earlier program stay
    /// visible, which is sound because a compiled program can only reference the
    /// names its own schema declared, and those are all rebound before it runs.
    ///
    /// The program itself evaluates in a child of that scope so its own
    /// intermediate bindings are discarded. Leaving them in the shared scope would
    /// let one program's local shadow a builtin another program calls.
    ///
    /// Crate-internal because that reasoning rests on the caller compiling its
    /// programs against a declared schema. A caller free to reference any name
    /// could read one left behind by an earlier call instead of being told it is
    /// unbound.
    pub(crate) fn evaluate_values<'a>(
        &mut self,
        program: &Program,
        bindings: impl IntoIterator<Item = (&'a str, Value)>,
    ) -> Result<Value, Diagnostic> {
        self.steps = 0;
        self.rng = ChaCha20Rng::seed_from_u64(self.config.seed);
        count!(Programs);
        for (name, value) in bindings {
            self.bindings.rebind(name, value);
        }
        let environment = self.bindings.child();
        self.eval_program(program, &environment)
            .map(|output| output.value)
    }
}

pub(crate) fn builtin_signatures() -> Vec<crate::squiggle::lint::BuiltinSignature> {
    builtin::signatures()
}

/// Returns every callable name registered by the core Squiggle runtime.
pub fn builtin_names() -> Vec<&'static str> {
    standard::builtin_names()
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), String>;

    /// Bound programs share one scope, so one must not disturb the next.
    ///
    /// A projection evaluates every node equation on the same runtime, and the
    /// scope keeps whatever each of them bound. What makes that safe is that the
    /// program itself runs one level down: a local here is called `min`, which
    /// would break the next program's call to the builtin if the two shared a
    /// frame.
    #[test]
    fn isolates_bound_programs_that_share_one_scope() -> TestResult {
        let mut runtime = Runtime::new();
        let shadows = parse("min = 3\nmin + baseline").map_err(|error| format!("{error:?}"))?;
        let calls_builtin = parse("min([baseline, 10])").map_err(|error| format!("{error:?}"))?;

        for _ in 0..2 {
            let shadowed = runtime
                .evaluate_values(&shadows, [("baseline", Value::Number(4.0))])
                .map_err(|error| error.message.clone())?;
            assert_eq!(shadowed.as_number(), Some(7.0));
            let builtin = runtime
                .evaluate_values(&calls_builtin, [("baseline", Value::Number(4.0))])
                .map_err(|error| error.message.clone())?;
            assert_eq!(builtin.as_number(), Some(4.0));
        }
        Ok(())
    }

    /// Rebinding a name the scope already holds must replace it, not accumulate.
    #[test]
    fn rebinds_a_name_the_scope_already_holds() -> TestResult {
        let mut runtime = Runtime::new();
        let program = parse("baseline * 2").map_err(|error| format!("{error:?}"))?;
        for value in [1.0, 5.0, 2.5] {
            let result = runtime
                .evaluate_values(&program, [("baseline", Value::Number(value))])
                .map_err(|error| error.message.clone())?;
            assert_eq!(result.as_number(), Some(value * 2.0));
        }
        Ok(())
    }

    fn evaluate(source: &str) -> Result<Value, String> {
        Runtime::new()
            .evaluate(source)
            .map_err(|errors| format!("{errors:?}"))
    }

    fn evaluate_with(runtime: &mut Runtime, source: &str) -> Result<Value, String> {
        runtime
            .evaluate(source)
            .map_err(|errors| format!("{errors:?}"))
    }

    fn array(value: Value) -> Result<Vec<Value>, String> {
        if let Value::Array(values) = value {
            Ok(values)
        } else {
            Err(format!("expected Array, received {}", value.type_name()))
        }
    }

    fn number(values: &[Value], index: usize) -> Result<f64, String> {
        values
            .get(index)
            .and_then(Value::as_number)
            .ok_or_else(|| format!("missing Number at index {index}"))
    }

    #[test]
    fn evaluates_closures_and_precedence() -> TestResult {
        let value = evaluate("x = 2\nf(y) = x + y * 3\nf(4)")?;
        assert_eq!(value, Value::Number(14.0));
        Ok(())
    }

    #[test]
    fn evaluates_recursive_lazy_functions() -> TestResult {
        let source = "factorial(n) = if n <= 1 then 1 else n * factorial(n - 1)\nfactorial(6)";
        assert_eq!(evaluate(source)?, Value::Number(720.0));
        Ok(())
    }

    #[test]
    fn evaluates_units_collections_and_lookups() -> TestResult {
        let source = "values = [2k, 3]\nrecord = {values: values, enabled: true}\nrecord.values[0]";
        assert_eq!(evaluate(source)?, Value::Number(2_000.0));
        Ok(())
    }

    #[test]
    fn constructs_a_ninety_percent_interval() -> TestResult {
        let value = evaluate("5 to 10")?;
        let distribution = value
            .as_distribution()
            .ok_or_else(|| "expected Distribution".to_owned())?;
        assert!((distribution.quantile(0.05)? - 5.0).abs() < 1e-8);
        assert!((distribution.quantile(0.95)? - 10.0).abs() < 1e-8);
        Ok(())
    }

    #[test]
    fn reports_runtime_spans() -> TestResult {
        let diagnostics = Runtime::new()
            .evaluate("missing + 1")
            .err()
            .ok_or_else(|| "invalid program evaluated successfully".to_owned())?;
        let diagnostic = diagnostics
            .into_iter()
            .next()
            .ok_or_else(|| "runtime returned no diagnostic".to_owned())?;
        assert!(diagnostic.message.contains("missing"));
        assert_eq!(diagnostic.span.start, 0);
        Ok(())
    }

    #[test]
    fn evaluates_distribution_constructors_and_statistics() -> TestResult {
        let source = "estimate = normal({p5: 4, p95: 10})\n[quantile(estimate, 0.05), mean(estimate), quantile(estimate, 0.95)]";
        let values = array(evaluate(source)?)?;
        assert!((number(&values, 0)? - 4.0).abs() < 1e-8);
        assert_eq!(number(&values, 1)?, 7.0);
        assert!((number(&values, 2)? - 10.0).abs() < 1e-8);
        Ok(())
    }

    #[test]
    fn distribution_algebra_is_seeded_and_statistically_consistent() -> TestResult {
        let config = RuntimeConfig {
            seed: 91,
            sample_count: 20_000,
            max_steps: 1_000_000,
        };
        let source = "mean(normal(5, 2) + normal(10, 3))";
        let first = evaluate_with(&mut Runtime::with_config(config)?, source)?;
        let second = evaluate_with(&mut Runtime::with_config(config)?, source)?;
        assert_eq!(first, second);
        let result = first
            .as_number()
            .ok_or_else(|| "expected Number result".to_owned())?;
        let error = (result - 15.0).abs();
        let standard_error = (13.0_f64 / config.sample_count as f64).sqrt();
        assert!(error < 5.0 * standard_error);
        Ok(())
    }

    #[test]
    fn evaluates_higher_order_collections_and_namespaces() -> TestResult {
        let source = "mapped = List.map([1, 2, 3], {|x, index| x + index})\nDict.set({values: mapped}, 'total', sum(mapped)).total";
        assert_eq!(evaluate(source)?, Value::Number(9.0));
        Ok(())
    }

    #[test]
    fn samples_replay_for_the_same_runtime_seed() -> TestResult {
        let source = "sampleN(uniform(0, 1), 5)";
        let mut runtime = Runtime::with_config(RuntimeConfig {
            seed: 7,
            sample_count: 100,
            max_steps: 1000,
        })?;
        let first = evaluate_with(&mut runtime, source)?;
        let second = evaluate_with(&mut runtime, source)?;
        assert_eq!(first, second);
        Ok(())
    }

    #[test]
    fn evaluates_statement_bearing_unit_annotated_lambdas() -> TestResult {
        let source =
            "apply(fn, value) = fn(value)\napply({|x :: usd| doubled = x * 2; doubled} :: usd, 4)";
        assert_eq!(evaluate(source)?, Value::Number(8.0));
        Ok(())
    }

    #[test]
    fn transforms_sample_sets_and_scores_forecasts() -> TestResult {
        let source = "samples = SampleSet.fromList([1, 2, 3])\nshifted = SampleSet.map(samples, {|x| x + 2})\n[mean(shifted), Dist.klDivergence(Sym.normal(0, 1), Sym.normal(0, 1))]";
        let values = array(evaluate(source)?)?;
        assert_eq!(number(&values, 0)?, 4.0);
        assert!(number(&values, 1)?.abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn evaluates_dates_durations_and_suffixes() -> TestResult {
        let source = "start = Date.make('2020-01-01')\nfinish = start + 2days\n[Date.toUnixTime(finish) - Date.toUnixTime(start), Duration.toHours(90minutes)]";
        let values = array(evaluate(source)?)?;
        assert_eq!(values, vec![Value::Number(172_800.0), Value::Number(1.5)]);
        Ok(())
    }

    #[test]
    fn evaluates_common_string_list_and_dictionary_utilities() -> TestResult {
        let source = "fallback = try({|| throw('no')}, {|| 42}).value\nordered = List.sortBy([{score: 3}, {score: 1}], {|item| item.score})\ntext = String.make(ordered[0].score)\n[fallback, text, Dict.pick({a: 1, b: 2}, ['b']).b, typeOf(ordered)]";
        let values = array(evaluate(source)?)?;
        assert_eq!(
            values,
            vec![
                Value::Number(42.0),
                Value::String("1".into()),
                Value::Number(2.0),
                Value::String("Array".into())
            ]
        );
        Ok(())
    }

    #[test]
    fn validates_parameter_domains_and_returns_exports() -> TestResult {
        let source = "export bounded(x: Number.rangeDomain(0, 10)) = x * 2\nbounded(4)";
        let output = Runtime::new()
            .evaluate_module(source)
            .map_err(|errors| format!("{errors:?}"))?;
        assert_eq!(output.value, Value::Number(8.0));
        assert!(matches!(
            output.exports.get("bounded"),
            Some(Value::Function(_))
        ));
        assert!(
            Runtime::new()
                .evaluate("bounded(x: Number.rangeDomain(0, 1)) = x\nbounded(2)")
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn builtin_declarations_enforce_arity_types_and_constraints() -> TestResult {
        for (source, expected) in [
            ("typeOf()", "typeOf(value: *)"),
            ("Date.toUnixTime(1)", "Date.toUnixTime(value: Date)"),
            (
                "sampleN(uniform(0, 1), 1.5)",
                "sampleN(distribution: Distribution, count: NonNegativeInteger)",
            ),
        ] {
            let diagnostics = Runtime::new()
                .evaluate(source)
                .err()
                .ok_or_else(|| format!("{source} unexpectedly succeeded"))?;
            let message = diagnostics
                .first()
                .map(|diagnostic| diagnostic.message.as_str())
                .ok_or_else(|| format!("{source} returned no diagnostic"))?;
            assert!(message.contains(expected), "{message}");
        }
        Ok(())
    }

    #[test]
    fn variadic_builtin_declarations_enforce_element_types() -> TestResult {
        assert_eq!(
            evaluate("concat('a', 'b', 'c')")?,
            Value::String("abc".into())
        );
        assert_eq!(evaluate("concat()")?, Value::String(String::new()));
        let diagnostics = Runtime::new()
            .evaluate("concat('a', 2)")
            .err()
            .ok_or_else(|| "invalid variadic call unexpectedly succeeded".to_owned())?;
        let message = diagnostics
            .first()
            .map(|diagnostic| diagnostic.message.as_str())
            .ok_or_else(|| "invalid variadic call returned no diagnostic".to_owned())?;
        assert!(message.contains("concat(...values: String)"), "{message}");
        assert_eq!(evaluate("mean(mixture(1, 1))")?, Value::Number(1.0));
        let diagnostics = Runtime::new()
            .evaluate("mixture()")
            .err()
            .ok_or_else(|| "empty prefixed variadic call unexpectedly succeeded".to_owned())?;
        let message = diagnostics
            .first()
            .map(|diagnostic| diagnostic.message.as_str())
            .ok_or_else(|| "empty prefixed variadic call returned no diagnostic".to_owned())?;
        assert!(
            message.contains("mixture(first: *, ...rest: *)"),
            "{message}"
        );
        Ok(())
    }

    #[test]
    fn composite_builtin_declarations_enforce_nested_and_union_types() -> TestResult {
        for (source, expected) in [
            ("mean([1, 'two'])", "mean(values: [Number])"),
            (
                "Dict.pick({a: 1}, ['a', 2])",
                "Dict.pick(values: Dictionary, keys: [String])",
            ),
            (
                "PointSet.make('invalid')",
                "PointSet.make(value: (Number | Distribution))",
            ),
        ] {
            let diagnostics = Runtime::new()
                .evaluate(source)
                .err()
                .ok_or_else(|| format!("{source} unexpectedly succeeded"))?;
            let message = diagnostics
                .first()
                .map(|diagnostic| diagnostic.message.as_str())
                .ok_or_else(|| format!("{source} returned no diagnostic"))?;
            assert!(message.contains(expected), "{message}");
        }
        Ok(())
    }
}
