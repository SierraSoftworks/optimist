use chumsky::prelude::*;

use crate::squiggle::{
    ast::{Expression, ExpressionKind, Span, Statement},
    token::Token,
};

pub(super) type ParseError<'tokens> = Rich<'tokens, Token, SimpleSpan<usize>>;

#[derive(Clone)]
pub(super) enum BodyItem {
    Statement(Statement),
    Expression(Expression),
}

pub(super) fn span(value: SimpleSpan<usize>) -> Span {
    Span {
        start: value.start,
        end: value.end,
    }
}

pub(super) fn expression(kind: ExpressionKind, value: SimpleSpan<usize>) -> Expression {
    Expression {
        kind,
        span: span(value),
    }
}

pub(super) fn binary(
    left: Expression,
    operator: String,
    right: Expression,
    value: SimpleSpan<usize>,
) -> Expression {
    expression(
        ExpressionKind::Binary {
            operator,
            left: Box::new(left),
            right: Box::new(right),
        },
        value,
    )
}

pub(super) fn body<'tokens>(
    items: Vec<BodyItem>,
    value: SimpleSpan<usize>,
) -> Result<Expression, ParseError<'tokens>> {
    let mut statements = Vec::new();
    let mut result = None;
    for item in items {
        match item {
            BodyItem::Statement(statement) if result.is_none() => statements.push(statement),
            BodyItem::Expression(expr) if result.is_none() => result = Some(expr),
            _ => {
                return Err(Rich::custom(
                    value,
                    "a block result must be its final expression",
                ));
            }
        }
    }
    result
        .map(|result| {
            expression(
                ExpressionKind::Block {
                    statements,
                    result: Box::new(result),
                },
                value,
            )
        })
        .ok_or_else(|| Rich::custom(value, "a block must end with an expression"))
}
