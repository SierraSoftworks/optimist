use chumsky::prelude::*;

use super::token::Token;

pub(crate) type LexicalError<'src> = Rich<'src, char, SimpleSpan<usize>>;
pub(crate) type SpannedToken = (Token, SimpleSpan<usize>);

pub(crate) fn lexer<'src>()
-> impl Parser<'src, &'src str, Vec<SpannedToken>, extra::Err<LexicalError<'src>>> {
    let digits = text::digits(10).to_slice();
    let fraction = just('.').then(digits.or_not());
    let exponent = one_of("eE").then(one_of("+-").or_not()).then(digits);
    let unit = choice((
        just("minutes"),
        just("hours"),
        just("days"),
        just("years"),
        just("year"),
        just("n"),
        just("m"),
        just("k"),
        just("M"),
        just("B"),
        just("G"),
        just("T"),
        just("P"),
        just("%"),
    ))
    .to_slice();
    let number = choice((
        text::int(10)
            .then(fraction.or_not())
            .then(exponent.or_not())
            .ignored(),
        just('.').then(digits).then(exponent.or_not()).ignored(),
    ))
    .to_slice()
    .then(unit.or_not())
    .try_map(|(raw, unit): (&str, Option<&str>), span| {
        raw.parse::<f64>()
            .map(|value| Token::Number(value, unit.map(str::to_owned)))
            .map_err(|_| Rich::custom(span, "invalid number"))
    });

    let escape =
        just('\\').ignore_then(choice((
            just('n').to('\n'),
            just('r').to('\r'),
            just('t').to('\t'),
            just('\\').to('\\'),
            just('\'').to('\''),
            just('"').to('"'),
            just('u').ignore_then(text::digits(16).exactly(4).to_slice().try_map(
                |digits, span| {
                    u32::from_str_radix(digits, 16)
                        .ok()
                        .and_then(char::from_u32)
                        .ok_or_else(|| Rich::custom(span, "invalid Unicode escape"))
                },
            )),
        )));
    let string = |quote| {
        none_of(['\\', quote])
            .or(escape)
            .repeated()
            .collect::<String>()
            .delimited_by(just(quote), just(quote))
            .map(Token::String)
    };

    let identifier = one_of("$_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ")
        .then(one_of("$_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789").repeated())
        .to_slice()
        .map(|name: &str| match name {
            "true" => Token::True,
            "false" => Token::False,
            "import" => Token::Import,
            "as" => Token::As,
            "export" => Token::Export,
            "if" => Token::If,
            "then" => Token::Then,
            "else" => Token::Else,
            "to" => Token::To,
            _ => Token::Identifier(name.to_owned()),
        });
    let operator = choice((
        just("=="),
        just("!="),
        just("<="),
        just(">="),
        just("&&"),
        just("||"),
        just(".+"),
        just(".-"),
        just(".*"),
        just("./"),
        just(".^"),
        just("+"),
        just("-"),
        just("*"),
        just("/"),
        just("^"),
        just("<"),
        just(">"),
        just("!"),
    ))
    .to_slice()
    .map(|value: &str| Token::Operator(value.to_owned()));
    let structural = choice((just("->").to(Token::Arrow), just("::").to(Token::Unit)));
    let control = choice((
        just('(').to(Token::LeftParen),
        just(')').to(Token::RightParen),
        just('[').to(Token::LeftBracket),
        just(']').to(Token::RightBracket),
        just('{').to(Token::LeftBrace),
        just('}').to(Token::RightBrace),
        just(',').to(Token::Comma),
        just(':').to(Token::Colon),
        just('?').to(Token::Question),
        just('.').to(Token::Dot),
        just('@').to(Token::At),
        just('|').to(Token::Pipe),
        just('=').to(Token::Assign),
        just(';').to(Token::Separator),
        just('\n').to(Token::Separator),
        just('\r').then(just('\n').or_not()).to(Token::Separator),
    ));
    let line_comment = just("//").then(none_of("\r\n").repeated()).ignored();
    let block_comment = just("/*")
        .then(any().and_is(just("*/").not()).repeated())
        .then_ignore(just("*/"))
        .ignored();
    let padding = one_of(" \t")
        .ignored()
        .or(line_comment)
        .or(block_comment)
        .repeated();

    let token = choice((
        number,
        string('\''),
        string('"'),
        identifier,
        structural,
        operator,
        control,
    ))
    .map_with(|token, emitter| (token, emitter.span()))
    .padded_by(padding);

    token.repeated().collect().padded_by(padding)
}
