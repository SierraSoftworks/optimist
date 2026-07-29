use std::collections::BTreeMap;

use super::{ModuleOutput, Runtime, builtin};
use crate::squiggle::{
    Diagnostic, DurationValue, Value,
    ast::{BinaryOperator, Expression, ExpressionKind, Program, Span, Statement},
    value::{Environment, Function, FunctionKind},
};

impl Runtime {
    pub(super) fn eval_program(
        &mut self,
        program: &Program,
        environment: &Environment,
    ) -> Result<ModuleOutput, Diagnostic> {
        for import in &program.imports {
            let value = self.modules.get(&import.path).cloned().ok_or_else(|| {
                Diagnostic::runtime(
                    format!("module '{}' is not registered", import.path),
                    import.span,
                )
                .with_help("register the module on Runtime before evaluating this program")
            })?;
            environment.define(&import.name, value);
        }
        for statement in &program.statements {
            self.eval_statement(statement, environment)?;
        }
        let value = program.result.as_ref().map_or(Ok(Value::Void), |result| {
            self.eval_expr(result, environment)
        })?;
        let exports = program
            .statements
            .iter()
            .filter(|statement| statement.exported)
            .filter_map(|statement| {
                environment
                    .get(&statement.name)
                    .map(|value| (statement.name.clone(), value))
            })
            .collect();
        Ok(ModuleOutput { value, exports })
    }

    fn eval_statement(
        &mut self,
        statement: &Statement,
        environment: &Environment,
    ) -> Result<(), Diagnostic> {
        let mut value = match &statement.value.kind {
            ExpressionKind::Lambda {
                parameters, body, ..
            } => Value::Function(Function::user(
                Some(statement.name.clone()),
                parameters.clone(),
                body.as_ref().clone(),
                environment.snapshot(),
            )),
            _ => self.eval_expr(&statement.value, environment)?,
        };
        for decorator in statement.decorators.iter().rev() {
            let function = environment.get(&decorator.name).ok_or_else(|| {
                Diagnostic::runtime(
                    format!("unknown decorator '{}'", decorator.name),
                    decorator.span,
                )
            })?;
            let mut arguments = vec![value];
            for argument in &decorator.arguments {
                arguments.push(self.eval_expr(argument, environment)?);
            }
            value = self.call(function, &arguments, decorator.span)?;
        }
        environment.define(&statement.name, value);
        Ok(())
    }

    pub(super) fn eval_expr(
        &mut self,
        expression: &Expression,
        environment: &Environment,
    ) -> Result<Value, Diagnostic> {
        self.step(expression.span)?;
        match &expression.kind {
            ExpressionKind::Number(value, unit) => {
                scale_unit(*value, unit.as_deref(), expression.span)
            }
            ExpressionKind::Boolean(value) => Ok(Value::Boolean(*value)),
            ExpressionKind::String(value) => Ok(Value::String(value.clone())),
            ExpressionKind::Variable(name) => environment.get(name).ok_or_else(|| {
                Diagnostic::runtime(format!("unknown identifier '{name}'"), expression.span)
            }),
            ExpressionKind::Array(items) => items
                .iter()
                .map(|item| self.eval_expr(item, environment))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            ExpressionKind::Dictionary(entries) => {
                self.eval_dictionary(entries, environment, expression.span)
            }
            ExpressionKind::Lambda {
                parameters, body, ..
            } => Ok(Value::Function(Function::user(
                None,
                parameters.clone(),
                body.as_ref().clone(),
                environment.snapshot(),
            ))),
            ExpressionKind::Block { statements, result } => {
                let child = environment.child();
                for statement in statements {
                    self.eval_statement(statement, &child)?;
                }
                self.eval_expr(result, &child)
            }
            ExpressionKind::Unary {
                operator,
                expression: value,
            } => {
                let value = self.eval_expr(value, environment)?;
                self.unary(*operator, value, expression.span)
            }
            ExpressionKind::Binary {
                operator,
                left,
                right,
            } => self.eval_binary(*operator, left, right, environment, expression.span),
            ExpressionKind::Conditional {
                condition,
                when_true,
                when_false,
            } => match self.eval_expr(condition, environment)? {
                Value::Boolean(true) => self.eval_expr(when_true, environment),
                Value::Boolean(false) => self.eval_expr(when_false, environment),
                value => Err(type_error("conditional", "Boolean", &value, condition.span)),
            },
            ExpressionKind::Call {
                function,
                arguments,
            } => {
                let function = self.eval_expr(function, environment)?;
                self.applied(function, arguments, None, environment, expression.span)
            }
            ExpressionKind::Lookup { value, key } => {
                // `a.b` parses to a lookup with a literal key, so the key is
                // known without building a String value to throw away.
                if let ExpressionKind::String(name) = &key.kind {
                    self.step(key.span)?;
                    let value = self.eval_expr(value, environment)?;
                    return field(value, name, expression.span);
                }
                let value = self.eval_expr(value, environment)?;
                let key = self.eval_expr(key, environment)?;
                lookup(value, key, expression.span)
            }
            ExpressionKind::Pipe {
                value,
                function,
                arguments,
            } => {
                let function = self.eval_expr(function, environment)?;
                let piped = self.eval_expr(value, environment)?;
                self.applied(function, arguments, Some(piped), environment, expression.span)
            }
        }
    }

    /// Evaluates the arguments of a call and applies `function` to them.
    ///
    /// The arguments go into a buffer this runtime lends out and takes back,
    /// rather than a fresh allocation per call, because builtins only ever read
    /// the list. A buffer abandoned by a failing argument is simply not returned:
    /// a diagnostic ends the whole run, so the pool refilling afterwards costs
    /// nothing worth branching for on the path that succeeds.
    ///
    /// A pool rather than one shared stack because a builtin may call back into
    /// the evaluator — `List.map` does — while its own arguments are still
    /// borrowed.
    fn applied(
        &mut self,
        function: Value,
        arguments: &[Expression],
        piped: Option<Value>,
        environment: &Environment,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        let mut values = self.buffers.pop().unwrap_or_default();
        values.extend(piped);
        for argument in arguments {
            values.push(self.eval_expr(argument, environment)?);
        }
        let result = self.call(function, &values, span);
        values.clear();
        self.buffers.push(values);
        result
    }

    fn eval_binary(
        &mut self,
        operator: BinaryOperator,
        left: &Expression,
        right: &Expression,
        environment: &Environment,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        let left = self.eval_expr(left, environment)?;
        if let BinaryOperator::And | BinaryOperator::Or = operator {
            let Value::Boolean(left) = left else {
                return Err(type_error(operator.spelling(), "Boolean", &left, span));
            };
            if left != (operator == BinaryOperator::And) {
                return Ok(Value::Boolean(left));
            }
        }
        let right = self.eval_expr(right, environment)?;
        self.binary(operator, left, right, span)
    }

    pub(super) fn call(
        &mut self,
        function: Value,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, Diagnostic> {
        self.step(span)?;
        let Value::Function(function) = function else {
            return Err(type_error("call", "Function", &function, span));
        };
        // Borrowed rather than taken: a user function owns its parameters, its
        // whole body, and the scope it closed over, so matching by value copied
        // an entire syntax tree on every call.
        match &*function.0 {
            FunctionKind::Builtin(name) => builtin::call(self, name, arguments, span),
            FunctionKind::User {
                name,
                parameters,
                body,
                environment,
            } => {
                if parameters.len() != arguments.len() {
                    return Err(Diagnostic::runtime(
                        format!(
                            "expected {} arguments, received {}",
                            parameters.len(),
                            arguments.len()
                        ),
                        span,
                    ));
                }
                let call_environment = environment.child();
                if let Some(name) = name {
                    call_environment.define(name.as_str(), Value::Function(function.clone()));
                }
                for (parameter, argument) in parameters.iter().zip(arguments) {
                    call_environment.define(&parameter.name, argument.clone());
                }
                for (parameter, argument) in parameters.iter().zip(arguments) {
                    let Some(annotation) = &parameter.annotation else {
                        continue;
                    };
                    let validation = self.eval_expr(annotation, environment)?;
                    let valid = match validation {
                        Value::Domain(domain) => domain.contains(argument),
                        Value::Function(_) => {
                            match self.call(validation, std::slice::from_ref(argument), parameter.span)? {
                                Value::Boolean(valid) => valid,
                                value => {
                                    return Err(type_error(
                                        "parameter annotation",
                                        "Boolean callback result",
                                        &value,
                                        parameter.span,
                                    ));
                                }
                            }
                        }
                        Value::Boolean(valid) => valid,
                        value => {
                            return Err(type_error(
                                "parameter annotation",
                                "Domain or Function",
                                &value,
                                parameter.span,
                            ));
                        }
                    };
                    if !valid {
                        return Err(Diagnostic::runtime(
                            format!(
                                "argument '{}' is outside its declared domain",
                                parameter.name
                            ),
                            parameter.span,
                        ));
                    }
                }
                self.eval_expr(body, &call_environment)
            }
        }
    }

    fn eval_dictionary(
        &mut self,
        entries: &[(Expression, Expression)],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        let mut result = BTreeMap::new();
        for (key, value) in entries {
            let key = self.eval_expr(key, environment)?;
            let Value::String(key) = key else {
                return Err(type_error("dictionary key", "String", &key, span));
            };
            result.insert(key, self.eval_expr(value, environment)?);
        }
        Ok(Value::dictionary(result))
    }

    fn step(&mut self, span: Span) -> Result<(), Diagnostic> {
        self.steps += 1;
        (self.steps <= self.config.max_steps)
            .then_some(())
            .ok_or_else(|| {
                Diagnostic::runtime("evaluation step limit exceeded", span).with_help(
                    "raise RuntimeConfig::max_steps for intentionally expensive programs",
                )
            })
    }
}

fn field(value: Value, name: &str, span: Span) -> Result<Value, Diagnostic> {
    match value {
        Value::Dictionary(values) => values
            .get(name)
            .cloned()
            .ok_or_else(|| Diagnostic::runtime(format!("dictionary has no key '{name}'"), span)),
        value => Err(Diagnostic::runtime(
            format!("cannot index {} with String", value.type_name()),
            span,
        )),
    }
}

fn lookup(value: Value, key: Value, span: Span) -> Result<Value, Diagnostic> {
    match (value, key) {
        (Value::Dictionary(values), Value::String(key)) => values
            .get(&key)
            .cloned()
            .ok_or_else(|| Diagnostic::runtime(format!("dictionary has no key '{key}'"), span)),
        (Value::Array(values), Value::Number(index)) if index >= 0.0 && index.fract() == 0.0 => {
            values
                .get(index as usize)
                .cloned()
                .ok_or_else(|| Diagnostic::runtime("array index is out of bounds", span))
        }
        (value, key) => Err(Diagnostic::runtime(
            format!(
                "cannot index {} with {}",
                value.type_name(),
                key.type_name()
            ),
            span,
        )),
    }
}

fn scale_unit(value: f64, unit: Option<&str>, span: Span) -> Result<Value, Diagnostic> {
    let scale = match unit {
        None => 1.0,
        Some("n") => 1e-9,
        Some("m") => 1e-3,
        Some("%") => 1e-2,
        Some("k") => 1e3,
        Some("M") => 1e6,
        Some("B" | "G") => 1e9,
        Some("T") => 1e12,
        Some("P") => 1e15,
        Some("minutes") => {
            return DurationValue::from_minutes(value)
                .map(Value::Duration)
                .map_err(|error| Diagnostic::runtime(error, span));
        }
        Some("hours") => {
            return DurationValue::from_hours(value)
                .map(Value::Duration)
                .map_err(|error| Diagnostic::runtime(error, span));
        }
        Some("days") => {
            return DurationValue::from_days(value)
                .map(Value::Duration)
                .map_err(|error| Diagnostic::runtime(error, span));
        }
        Some("year" | "years") => {
            return DurationValue::from_years(value)
                .map(Value::Duration)
                .map_err(|error| Diagnostic::runtime(error, span));
        }
        Some(unit) => {
            return Err(Diagnostic::runtime(
                format!("unit suffix '{unit}' has no core numeric conversion"),
                span,
            ));
        }
    };
    Ok(Value::Number(value * scale))
}

fn type_error(operation: &str, expected: &str, value: &Value, span: Span) -> Diagnostic {
    Diagnostic::runtime(
        format!(
            "{operation} expected {expected}, received {}",
            value.type_name()
        ),
        span,
    )
}
