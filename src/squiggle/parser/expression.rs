use chumsky::{input::ValueInput, prelude::*};

use crate::squiggle::{
    ast::{Expression, ExpressionKind},
    token::Token,
};

use super::{
    common::{ParseError, binary, expression},
    term::term_parser,
};

fn left_assoc<'tokens, I, P>(
    base: P,
    operators: &'static [&'static str],
) -> impl Parser<'tokens, I, Expression, extra::Err<ParseError<'tokens>>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan<usize>>,
    P: Parser<'tokens, I, Expression, extra::Err<ParseError<'tokens>>> + Clone + 'tokens,
{
    base.clone().foldl_with(
        select! { Token::Operator(operator) if operators.contains(&operator.as_str()) => operator }
            .then_ignore(just(Token::Separator).repeated()).then(base).repeated(),
        |left, (operator, right), emitter| binary(left, operator, right, emitter.span()),
    ).boxed()
}

pub(super) fn expression_parser<'tokens, I>()
-> impl Parser<'tokens, I, Expression, extra::Err<ParseError<'tokens>>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan<usize>>,
{
    recursive(|expr| {
        let separators = just(Token::Separator).repeated().ignored();
        let term = term_parser(expr.clone());
        let power = recursive(|power| {
            term.clone().then(
            select! { Token::Operator(operator) if operator == "^" || operator == ".^" => operator }
                .then_ignore(separators.clone()).then(power).or_not(),
        ).map_with(|(left, tail), emitter| tail.map_or(left.clone(), |(operator, right)| {
            binary(left, operator, right, emitter.span())
        })).boxed()
        });
        let product = left_assoc(power, &["*", "/", ".*", "./"]);
        let sum = left_assoc(product, &["+", "-", ".+", ".-"]);
        let interval = sum
            .clone()
            .foldl_with(
                just(Token::To)
                    .to("to".to_owned())
                    .then_ignore(separators.clone())
                    .then(sum)
                    .repeated(),
                |left, (operator, right), emitter| binary(left, operator, right, emitter.span()),
            )
            .boxed();
        let relation = left_assoc(interval, &["<", "<=", ">", ">="]);
        let equality = left_assoc(relation, &["==", "!="]);
        let logical_and = left_assoc(equality, &["&&"]);
        let logical_or = left_assoc(logical_and, &["||"]);
        let ternary = logical_or
            .clone()
            .then(
                just(Token::Question)
                    .ignore_then(just(Token::Separator).repeated())
                    .ignore_then(expr.clone())
                    .then_ignore(just(Token::Colon))
                    .then_ignore(just(Token::Separator).repeated())
                    .then(expr.clone())
                    .or_not(),
            )
            .map_with(|(condition, branches), emitter| {
                branches.map_or(condition.clone(), |(when_true, when_false)| {
                    expression(
                        ExpressionKind::Conditional {
                            condition: Box::new(condition),
                            when_true: Box::new(when_true),
                            when_false: Box::new(when_false),
                        },
                        emitter.span(),
                    )
                })
            })
            .boxed();
        let linebreaks = just(Token::Separator).repeated();
        let if_then_else = just(Token::If)
            .ignore_then(linebreaks.clone())
            .ignore_then(logical_or)
            .then_ignore(linebreaks.clone())
            .then_ignore(just(Token::Then))
            .then_ignore(linebreaks.clone())
            .then(expr.clone())
            .then_ignore(linebreaks.clone())
            .then_ignore(just(Token::Else))
            .then_ignore(linebreaks)
            .then(expr)
            .map_with(|((condition, when_true), when_false), emitter| {
                expression(
                    ExpressionKind::Conditional {
                        condition: Box::new(condition),
                        when_true: Box::new(when_true),
                        when_false: Box::new(when_false),
                    },
                    emitter.span(),
                )
            })
            .boxed();
        if_then_else
            .or(ternary)
            .labelled("expression")
            .as_context()
            .boxed()
    })
}
