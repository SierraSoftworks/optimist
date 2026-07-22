use chumsky::{input::ValueInput, prelude::*};

use crate::squiggle::{
    ast::{Expression, ExpressionKind},
    token::Token,
};

use super::{
    common::{BodyItem, ParseError, body, expression},
    statement::{parameter_parser, statement_parser, unit_parser},
};

pub(super) fn atom_parser<'tokens, I, P>(
    expr: P,
) -> impl Parser<'tokens, I, Expression, extra::Err<ParseError<'tokens>>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan<usize>>,
    P: Parser<'tokens, I, Expression, extra::Err<ParseError<'tokens>>> + Clone + 'tokens,
{
    let separators = just(Token::Separator).repeated().ignored();
    let separator = just(Token::Separator).repeated().at_least(1).ignored();
    let identifier = select! { Token::Identifier(name) => name }.labelled("identifier");
    let arguments = expr
        .clone()
        .padded_by(separators.clone())
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>();
    let parameters = parameter_parser(expr.clone())
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .padded_by(separators.clone());
    let literal = choice((
        select! { Token::Number(value, unit) => ExpressionKind::Number(value, unit) },
        select! { Token::String(value) => ExpressionKind::String(value) },
        just(Token::True).to(ExpressionKind::Boolean(true)),
        just(Token::False).to(ExpressionKind::Boolean(false)),
        identifier.map(ExpressionKind::Variable),
    ))
    .map_with(|kind, emitter| expression(kind, emitter.span()));
    let array = arguments
        .clone()
        .map(ExpressionKind::Array)
        .delimited_by(just(Token::LeftBracket), just(Token::RightBracket))
        .map_with(|kind, emitter| expression(kind, emitter.span()));
    let dictionary_entry = expr
        .clone()
        .then_ignore(just(Token::Colon))
        .then(expr.clone().padded_by(separators.clone()))
        .map(|(mut key, value)| {
            if let ExpressionKind::Variable(name) = &key.kind {
                key.kind = ExpressionKind::String(name.clone());
            }
            (key, value)
        })
        .or(identifier.map_with(|name, emitter| {
            (
                expression(ExpressionKind::String(name.clone()), emitter.span()),
                expression(ExpressionKind::Variable(name), emitter.span()),
            )
        }));
    let dictionary = dictionary_entry
        .padded_by(separators.clone())
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .map(ExpressionKind::Dictionary)
        .delimited_by(just(Token::LeftBrace), just(Token::RightBrace))
        .map_with(|kind, emitter| expression(kind, emitter.span()));
    let item = statement_parser(expr.clone())
        .map(BodyItem::Statement)
        .or(expr.clone().map(BodyItem::Expression));
    let lambda_body = item
        .clone()
        .separated_by(separator.clone())
        .collect::<Vec<_>>()
        .try_map(body);
    let lambda_parameters = choice((
        just(Token::Operator("||".into())).to(Vec::new()),
        just(Token::Pipe)
            .ignore_then(parameters)
            .then_ignore(just(Token::Pipe)),
    ));
    let lambda = just(Token::LeftBrace)
        .ignore_then(separators.clone())
        .ignore_then(lambda_parameters)
        .then(lambda_body.padded_by(separators.clone()))
        .then_ignore(just(Token::RightBrace))
        .then(unit_parser())
        .map_with(|((parameters, body), return_unit), emitter| {
            expression(
                ExpressionKind::Lambda {
                    parameters,
                    body: Box::new(body),
                    return_unit,
                },
                emitter.span(),
            )
        });
    let block = item
        .separated_by(separator)
        .collect::<Vec<_>>()
        .try_map(body)
        .padded_by(separators.clone())
        .delimited_by(just(Token::LeftBrace), just(Token::RightBrace));
    let grouped = expr
        .padded_by(separators)
        .delimited_by(just(Token::LeftParen), just(Token::RightParen));
    choice((lambda, array, block, dictionary, grouped, literal)).boxed()
}
