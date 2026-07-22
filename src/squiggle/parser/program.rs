use chumsky::{input::ValueInput, prelude::*};

use crate::squiggle::{
    ast::{Import, Program},
    token::Token,
};

use super::{
    common::{BodyItem, ParseError, span},
    expression::expression_parser,
    statement::statement_parser,
};

pub(crate) fn program_parser<'tokens, I>()
-> impl Parser<'tokens, I, Program, extra::Err<ParseError<'tokens>>>
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan<usize>>,
{
    let separators = just(Token::Separator).repeated().ignored();
    let separator = just(Token::Separator).repeated().at_least(1).ignored();
    let import = just(Token::Import)
        .ignore_then(select! { Token::String(path) => path })
        .then_ignore(just(Token::As))
        .then(select! { Token::Identifier(name) => name })
        .map_with(|(path, name), emitter| Import {
            path,
            name,
            span: span(emitter.span()),
        })
        .then_ignore(separator.clone());
    let expr = expression_parser();
    let item = statement_parser(expr.clone())
        .map(BodyItem::Statement)
        .or(expr.map(BodyItem::Expression));
    import
        .repeated()
        .collect::<Vec<_>>()
        .then(
            item.separated_by(separator)
                .allow_trailing()
                .collect::<Vec<_>>(),
        )
        .padded_by(separators)
        .then_ignore(end())
        .try_map(|(imports, items), value| {
            let mut statements = Vec::new();
            let mut result = None;
            for item in items {
                match item {
                    BodyItem::Statement(statement) if result.is_none() => {
                        statements.push(statement)
                    }
                    BodyItem::Expression(expression) if result.is_none() => {
                        result = Some(expression)
                    }
                    _ => {
                        return Err(Rich::custom(
                            value,
                            "the module result must be its final expression",
                        ));
                    }
                }
            }
            Ok(Program {
                imports,
                statements,
                result,
            })
        })
}
