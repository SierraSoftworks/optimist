use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Token {
    Number(f64, Option<String>),
    String(String),
    Identifier(String),
    True,
    False,
    Import,
    As,
    Export,
    If,
    Then,
    Else,
    To,
    Operator(String),
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    Comma,
    Colon,
    Question,
    Dot,
    At,
    Pipe,
    Assign,
    Arrow,
    Unit,
    Separator,
}

impl fmt::Display for Token {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(value, unit) => {
                write!(formatter, "{value}{}", unit.as_deref().unwrap_or(""))
            }
            Self::String(value) => write!(formatter, "\"{value}\""),
            Self::Identifier(value) | Self::Operator(value) => formatter.write_str(value),
            Self::True => formatter.write_str("true"),
            Self::False => formatter.write_str("false"),
            Self::Import => formatter.write_str("import"),
            Self::As => formatter.write_str("as"),
            Self::Export => formatter.write_str("export"),
            Self::If => formatter.write_str("if"),
            Self::Then => formatter.write_str("then"),
            Self::Else => formatter.write_str("else"),
            Self::To => formatter.write_str("to"),
            Self::LeftParen => formatter.write_str("("),
            Self::RightParen => formatter.write_str(")"),
            Self::LeftBracket => formatter.write_str("["),
            Self::RightBracket => formatter.write_str("]"),
            Self::LeftBrace => formatter.write_str("{"),
            Self::RightBrace => formatter.write_str("}"),
            Self::Comma => formatter.write_str(","),
            Self::Colon => formatter.write_str(":"),
            Self::Question => formatter.write_str("?"),
            Self::Dot => formatter.write_str("."),
            Self::At => formatter.write_str("@"),
            Self::Pipe => formatter.write_str("|"),
            Self::Assign => formatter.write_str("="),
            Self::Arrow => formatter.write_str("->"),
            Self::Unit => formatter.write_str("::"),
            Self::Separator => formatter.write_str("statement separator"),
        }
    }
}
