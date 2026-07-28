//! Resolution of the names a component type's expressions may refer to.
//!
//! A manifest is only useful if a mistyped name fails when the catalogue loads
//! rather than when a solver is midway through a run. Every expression is
//! therefore parsed and its free identifiers collected, then checked against the
//! surface the evaluator will actually provide.
//!
//! The visible surface is deliberately small: the component's own properties and
//! channels, the reserved bindings describing time and prior state, the requests
//! arriving on inbound ports, the responses returning on outbound ports, and the
//! Squiggle standard library. Names bound inside the expression itself, by a
//! block binding or a lambda parameter, shadow that surface and are not treated
//! as free.
//!
//! A component cannot see what it publishes itself. `in.<port>` is what arrived
//! and `out.<port>` is what came back, so neither name can be confused for the
//! response the component is sending or the request it is making.

use std::collections::BTreeSet;

use crate::squiggle::{
    Diagnostic,
    ast::{Expression, ExpressionKind, Program, Statement},
    builtin_names, parse,
};

/// The current step's elapsed time, in seconds since the run began.
pub(super) const TIME: &str = "t";
/// The length of the current step, in seconds.
pub(super) const STEP: &str = "dt";
/// This component's channel values at the previous step.
pub(super) const PREVIOUS: &str = "prev";
/// The requests arriving on this component's inbound ports.
pub(super) const INBOUND: &str = "in";
/// The responses returning on this component's outbound ports.
pub(super) const OUTBOUND: &str = "out";
/// The signals currently travelling along a relationship.
pub(super) const SIGNAL: &str = "signal";
/// The request travelling from caller to callee along a relationship.
pub(super) const REQUEST: &str = "request";
/// The response returning from callee to caller along a relationship.
pub(super) const RESPONSE: &str = "response";

/// Bindings the evaluator supplies to every channel expression.
pub(super) const RESERVED: &[&str] = &[TIME, STEP, PREVIOUS, INBOUND, OUTBOUND];

/// Bindings the evaluator supplies to every mutator transform.
///
/// A mutator sees the flows passing through it rather than the components on
/// either end, which is what keeps it reusable on any relationship. Both
/// directions are visible from either transform, so a retry policy can raise
/// demand in response to the failures coming back.
pub(super) const MUTATOR_RESERVED: &[&str] = &[TIME, STEP, SIGNAL, REQUEST, RESPONSE];

/// Parses an expression and reports the names it expects to be given.
pub(super) fn free_names(source: &str) -> Result<BTreeSet<String>, Vec<Diagnostic>> {
    let program = parse(source)?;
    let mut free = BTreeSet::new();
    let mut bound = builtin_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    // A namespaced builtin is reached through its root, so `Queue.mm1Wait` makes
    // `Queue` a reference the evaluator resolves rather than one a manifest owes.
    let roots = bound
        .iter()
        .filter_map(|name| name.split_once('.').map(|(root, _)| root.to_owned()))
        .collect::<Vec<_>>();
    bound.extend(roots);
    bound.extend(["pi", "e", "infinity"].map(str::to_owned));
    visit_program(&program, &mut bound, &mut free);
    Ok(free)
}

fn visit_program(program: &Program, bound: &mut BTreeSet<String>, free: &mut BTreeSet<String>) {
    let mut scope = bound.clone();
    for import in &program.imports {
        scope.insert(import.name.clone());
    }
    for statement in &program.statements {
        visit_statement(statement, &mut scope, free);
    }
    if let Some(result) = &program.result {
        visit(result, &scope, free);
    }
}

fn visit_statement(
    statement: &Statement,
    bound: &mut BTreeSet<String>,
    free: &mut BTreeSet<String>,
) {
    visit(&statement.value, bound, free);
    bound.insert(statement.name.clone());
}

fn visit(expression: &Expression, bound: &BTreeSet<String>, free: &mut BTreeSet<String>) {
    match &expression.kind {
        ExpressionKind::Variable(name) => {
            if !bound.contains(name) {
                free.insert(name.clone());
            }
        }
        ExpressionKind::Number(..) | ExpressionKind::Boolean(_) | ExpressionKind::String(_) => {}
        ExpressionKind::Array(items) => {
            for item in items {
                visit(item, bound, free);
            }
        }
        ExpressionKind::Dictionary(entries) => {
            for (key, value) in entries {
                visit(key, bound, free);
                visit(value, bound, free);
            }
        }
        ExpressionKind::Lambda {
            parameters, body, ..
        } => {
            let mut scope = bound.clone();
            for parameter in parameters {
                scope.insert(parameter.name.clone());
            }
            visit(body, &scope, free);
        }
        ExpressionKind::Block { statements, result } => {
            let mut scope = bound.clone();
            for statement in statements {
                visit_statement(statement, &mut scope, free);
            }
            visit(result, &scope, free);
        }
        ExpressionKind::Unary { expression, .. } => visit(expression, bound, free),
        ExpressionKind::Binary { left, right, .. } => {
            visit(left, bound, free);
            visit(right, bound, free);
        }
        ExpressionKind::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            visit(condition, bound, free);
            visit(when_true, bound, free);
            visit(when_false, bound, free);
        }
        ExpressionKind::Call {
            function,
            arguments,
        } => {
            visit(function, bound, free);
            for argument in arguments {
                visit(argument, bound, free);
            }
        }
        // Only the container is a reference; the key names a field on it.
        ExpressionKind::Lookup { value, .. } => visit(value, bound, free),
        ExpressionKind::Pipe {
            value,
            function,
            arguments,
        } => {
            visit(value, bound, free);
            visit(function, bound, free);
            for argument in arguments {
                visit(argument, bound, free);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(source: &str) -> Vec<String> {
        free_names(source)
            .expect("parses")
            .into_iter()
            .collect::<Vec<_>>()
    }

    #[test]
    fn plain_references_are_reported() {
        assert_eq!(names("rate * wait"), vec!["rate", "wait"]);
    }

    #[test]
    fn builtins_are_not_free() {
        assert_eq!(names("Queue.mm1Wait(service, 0.5)"), vec!["service"]);
        assert_eq!(names("min([demand, 10])"), vec!["demand"]);
    }

    #[test]
    fn reserved_bindings_are_reported_so_they_can_be_allowed() {
        assert_eq!(
            names("prev.backlog + inbound.rate * dt"),
            ["dt", "inbound", "prev"].map(str::to_owned).to_vec()
        );
    }

    #[test]
    fn local_bindings_shadow_the_surface() {
        assert_eq!(names("x = rate * 2\nx + x"), vec!["rate"]);
    }

    #[test]
    fn lambda_parameters_are_bound_within_their_body() {
        assert_eq!(names("List.map([1, 2], {|x| x * scale})"), vec!["scale"]);
    }

    #[test]
    fn lookup_keys_are_not_references() {
        assert_eq!(names("prev.backlog"), vec!["prev"]);
    }

    #[test]
    fn syntax_errors_are_returned() {
        assert!(free_names("rate * ").is_err());
    }
}
