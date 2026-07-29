use chumsky::{input::ValueInput, prelude::*};

use crate::squiggle::{
    ast::{Expression, ExpressionKind, UnaryOperator},
    token::Token,
};

use super::{
    atom::atom_parser,
    common::{ParseError, expression},
};

enum Postfix {
    Call(Vec<Expression>),
    Index(Expression),
    Field(String),
}

pub(super) fn term_parser<'tokens, I, P>(
    expr: P,
) -> impl Parser<'tokens, I, Expression, extra::Err<ParseError<'tokens>>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan<usize>>,
    P: Parser<'tokens, I, Expression, extra::Err<ParseError<'tokens>>> + Clone + 'tokens,
{
    let separators = just(Token::Separator).repeated().ignored();
    let identifier = select! { Token::Identifier(name) => name };
    let arguments = expr
        .clone()
        .padded_by(separators.clone())
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>();
    let postfix = choice((
        arguments
            .delimited_by(just(Token::LeftParen), just(Token::RightParen))
            .map(Postfix::Call),
        expr.clone()
            .padded_by(separators.clone())
            .delimited_by(just(Token::LeftBracket), just(Token::RightBracket))
            .map(Postfix::Index),
        just(Token::Dot).ignore_then(identifier).map(Postfix::Field),
    ));
    let post = atom_parser(expr)
        .foldl_with(
            postfix.repeated(),
            |value, postfix, emitter| match postfix {
                Postfix::Call(arguments) => expression(
                    ExpressionKind::Call {
                        function: Box::new(value),
                        arguments,
                    },
                    emitter.span(),
                ),
                Postfix::Index(key) => expression(
                    ExpressionKind::Lookup {
                        value: Box::new(value),
                        key: Box::new(key),
                    },
                    emitter.span(),
                ),
                Postfix::Field(key) => expression(
                    ExpressionKind::Lookup {
                        value: Box::new(value),
                        key: Box::new(expression(ExpressionKind::String(key), emitter.span())),
                    },
                    emitter.span(),
                ),
            },
        )
        .boxed();
    let unary =
        recursive(|unary| {
            select! {
        Token::Operator(operator) if UnaryOperator::parse(&operator).is_some() => operator
    }.then_ignore(separators.clone()).then(unary).map_with(|(operator, value), emitter| expression(
        ExpressionKind::Unary {
            operator: UnaryOperator::parse(&operator).expect("a parsed prefix operator"),
            expression: Box::new(value),
        }, emitter.span(),
    )).or(post.clone()).boxed()
        });
    unary
        .clone()
        .foldl_with(
            separators
                .clone()
                .ignore_then(just(Token::Arrow))
                .then_ignore(separators)
                .ignore_then(post)
                .repeated(),
            |value, callable, emitter| match callable.kind {
                ExpressionKind::Call {
                    function,
                    arguments,
                } => expression(
                    ExpressionKind::Pipe {
                        value: Box::new(value),
                        function,
                        arguments,
                    },
                    emitter.span(),
                ),
                _ => expression(
                    ExpressionKind::Pipe {
                        value: Box::new(value),
                        function: Box::new(callable),
                        arguments: Vec::new(),
                    },
                    emitter.span(),
                ),
            },
        )
        .boxed()
}
