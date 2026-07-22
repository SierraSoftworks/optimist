use chumsky::{input::ValueInput, prelude::*};

use crate::squiggle::{
    ast::{Decorator, Expression, ExpressionKind, Parameter, Statement, UnitType},
    token::Token,
};

use super::common::{ParseError, span};

pub(super) fn unit_parser<'tokens, I>()
-> impl Parser<'tokens, I, Option<UnitType>, extra::Err<ParseError<'tokens>>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan<usize>>,
{
    let identifier = select! { Token::Identifier(name) => name };
    let factor = choice((
        identifier,
        select! { Token::Number(value, None) => value.to_string() },
    ))
    .then(
        just(Token::Operator("^".into()))
            .ignore_then(select! {
                Token::Number(value, None) => value
            })
            .or_not(),
    )
    .map(|(name, exponent)| UnitType::Factor {
        name,
        exponent: exponent.unwrap_or(1.0),
    });
    let body = factor.clone().foldl(
        choice((
            just(Token::Operator("*".into())).to(true),
            just(Token::Operator("/".into())).to(false),
        ))
        .then(factor)
        .repeated(),
        |left, (product, right)| {
            if product {
                UnitType::Product(Box::new(left), Box::new(right))
            } else {
                UnitType::Ratio(Box::new(left), Box::new(right))
            }
        },
    );
    just(Token::Unit).ignore_then(body).or_not()
}

pub(super) fn parameter_parser<'tokens, I, P>(
    expr: P,
) -> impl Parser<'tokens, I, Parameter, extra::Err<ParseError<'tokens>>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan<usize>>,
    P: Parser<'tokens, I, Expression, extra::Err<ParseError<'tokens>>> + Clone + 'tokens,
{
    select! { Token::Identifier(name) => name }
        .then(just(Token::Colon).ignore_then(expr).or_not())
        .then(unit_parser())
        .map_with(|((name, annotation), unit), emitter| Parameter {
            name,
            annotation,
            unit,
            span: span(emitter.span()),
        })
}

pub(super) fn statement_parser<'tokens, I, P>(
    expr: P,
) -> impl Parser<'tokens, I, Statement, extra::Err<ParseError<'tokens>>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan<usize>>,
    P: Parser<'tokens, I, Expression, extra::Err<ParseError<'tokens>>> + Clone + 'tokens,
{
    let separators = just(Token::Separator).repeated().ignored();
    let separator = just(Token::Separator).repeated().at_least(1).ignored();
    let identifier = select! { Token::Identifier(name) => name }.labelled("identifier");
    let parameters = parameter_parser(expr.clone())
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .padded_by(separators.clone());
    let arguments = expr
        .clone()
        .padded_by(separators.clone())
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>();
    let decorator = just(Token::At)
        .ignore_then(identifier)
        .then(
            arguments
                .delimited_by(just(Token::LeftParen), just(Token::RightParen))
                .or_not(),
        )
        .then_ignore(separator)
        .map_with(|(name, arguments), emitter| Decorator {
            name,
            arguments: arguments.unwrap_or_default(),
            span: span(emitter.span()),
        });
    let prefix = decorator
        .repeated()
        .collect::<Vec<_>>()
        .then(just(Token::Export).or_not().map(|value| value.is_some()))
        .then(identifier);
    let function = prefix
        .clone()
        .then(parameters.delimited_by(just(Token::LeftParen), just(Token::RightParen)))
        .then(unit_parser())
        .then_ignore(just(Token::Assign))
        .then_ignore(separators.clone())
        .then(expr.clone())
        .map_with(
            |(((((decorators, exported), name), parameters), unit), body), emitter| {
                let statement_span = span(emitter.span());
                Statement {
                    decorators,
                    exported,
                    name,
                    unit,
                    value: Expression {
                        kind: ExpressionKind::Lambda {
                            parameters,
                            body: Box::new(body),
                            return_unit: None,
                        },
                        span: statement_span,
                    },
                    span: statement_span,
                }
            },
        );
    let binding = prefix
        .then(unit_parser())
        .then_ignore(just(Token::Assign))
        .then_ignore(separators)
        .then(expr)
        .map_with(
            |((((decorators, exported), name), unit), value), emitter| Statement {
                decorators,
                exported,
                name,
                unit,
                value,
                span: span(emitter.span()),
            },
        );
    function.or(binding).labelled("binding").boxed()
}
