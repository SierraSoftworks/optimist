use std::collections::{BTreeMap, BTreeSet};

use crate::squiggle::{
    Diagnostic,
    ast::{Expression, ExpressionKind, Parameter, Program, Span, Statement, UnitType},
    runtime,
};

use super::{
    BuiltinSignature,
    types::{FunctionType, Type, Unit},
};

pub(super) struct Checker {
    pub(super) diagnostics: Vec<Diagnostic>,
    pub(super) scopes: Vec<BTreeMap<String, Type>>,
    pub(super) builtins: BTreeSet<&'static str>,
    pub(super) signatures: Vec<BuiltinSignature>,
}

impl Checker {
    pub(super) fn new() -> Self {
        let mut root = BTreeMap::new();
        root.insert("pi".into(), Type::number(Some(std::f64::consts::PI)));
        root.insert("e".into(), Type::number(Some(std::f64::consts::E)));
        root.insert("infinity".into(), Type::number(None));
        let builtins = runtime::builtin_names().into_iter().collect();
        Self {
            diagnostics: Vec::new(),
            scopes: vec![root],
            builtins,
            signatures: runtime::builtin_signatures(),
        }
    }

    pub(super) fn check(mut self, program: &Program) -> Vec<Diagnostic> {
        for import in &program.imports {
            self.define(import.name.clone(), Type::Unknown);
        }
        for statement in &program.statements {
            if let ExpressionKind::Lambda { parameters, .. } = &statement.value.kind {
                let parameters = parameters
                    .iter()
                    .map(|parameter| self.parameter_type(parameter))
                    .collect();
                self.define(
                    statement.name.clone(),
                    Type::Function(FunctionType {
                        parameters,
                        result: Box::new(Type::Unknown),
                    }),
                );
            }
        }
        for statement in &program.statements {
            self.check_statement(statement);
        }
        if let Some(result) = &program.result {
            self.infer(result);
        }
        self.diagnostics
    }

    pub(super) fn check_statement(&mut self, statement: &Statement) {
        for decorator in &statement.decorators {
            for argument in &decorator.arguments {
                self.infer(argument);
            }
            if self.lookup(&decorator.name).is_none() {
                self.report(
                    format!("unknown decorator '{}'", decorator.name),
                    decorator.span,
                );
            }
        }
        let inferred = match &statement.value.kind {
            ExpressionKind::Lambda {
                parameters,
                body,
                return_unit,
            } => self.infer_function(
                parameters,
                body,
                return_unit.as_ref().or(statement.unit.as_ref()),
            ),
            _ => {
                let inferred = self.infer(&statement.value);
                self.apply_unit(inferred, statement.unit.as_ref(), statement.span)
            }
        };
        self.define(statement.name.clone(), inferred);
    }

    pub(super) fn infer_function(
        &mut self,
        parameters: &[Parameter],
        body: &Expression,
        return_unit: Option<&UnitType>,
    ) -> Type {
        let parameter_types = parameters
            .iter()
            .map(|parameter| self.parameter_type(parameter))
            .collect::<Vec<_>>();
        self.scopes.push(BTreeMap::new());
        for (parameter, parameter_type) in parameters.iter().zip(&parameter_types) {
            self.define(parameter.name.clone(), parameter_type.clone());
            if let Some(annotation) = &parameter.annotation {
                self.infer(annotation);
            }
        }
        let result = self.infer(body);
        self.scopes.pop();
        Type::Function(FunctionType {
            parameters: parameter_types,
            result: Box::new(self.apply_unit(result, return_unit, body.span)),
        })
    }

    fn parameter_type(&self, parameter: &Parameter) -> Type {
        if let Some(unit) = &parameter.unit {
            return Type::Number {
                literal: None,
                unit: Unit::from_ast(unit),
            };
        }
        let annotation_name =
            parameter
                .annotation
                .as_ref()
                .and_then(|annotation| match &annotation.kind {
                    ExpressionKind::Call { function, .. } => Self::expression_name(function),
                    _ => Self::expression_name(annotation),
                });
        match annotation_name.as_deref() {
            Some("Number.rangeDomain") => Type::number(None),
            Some("Date.rangeDomain") => Type::Date,
            _ => Type::Unknown,
        }
    }

    fn apply_unit(&mut self, value: Type, unit: Option<&UnitType>, span: Span) -> Type {
        let Some(unit) = unit else { return value };
        let declared = Unit::from_ast(unit);
        match value {
            Type::Unknown => Type::Number {
                literal: None,
                unit: declared,
            },
            Type::Number { literal, unit } => {
                if unit != Unit::default() && unit != declared {
                    self.report(
                        format!("declared unit {declared} does not match inferred unit {unit}"),
                        span,
                    );
                }
                Type::Number {
                    literal,
                    unit: declared,
                }
            }
            Type::Distribution(unit) => {
                if unit != Unit::default() && unit != declared {
                    self.report(
                        format!("declared unit {declared} does not match inferred unit {unit}"),
                        span,
                    );
                }
                Type::Distribution(declared)
            }
            value => {
                self.report(
                    format!(
                        "unit signature requires Number or Distribution, received {}",
                        value.display_name()
                    ),
                    span,
                );
                value
            }
        }
    }

    pub(super) fn expression_name(expression: &Expression) -> Option<String> {
        match &expression.kind {
            ExpressionKind::Variable(name) => Some(name.clone()),
            ExpressionKind::Lookup { value, key } => match &key.kind {
                ExpressionKind::String(key) => {
                    Some(format!("{}.{}", Self::expression_name(value)?, key))
                }
                _ => None,
            },
            _ => None,
        }
    }

    pub(super) fn define(&mut self, name: String, value: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, value);
        }
    }

    pub(super) fn lookup(&self, name: &str) -> Option<Type> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
            .or_else(|| {
                (self.builtins.contains(name) && !name.contains('.'))
                    .then(|| Type::Builtin(name.into()))
            })
    }

    pub(super) fn report(&mut self, message: impl Into<String>, span: Span) {
        self.diagnostics.push(Diagnostic::lint(message, span));
    }
}
