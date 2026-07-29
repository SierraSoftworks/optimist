//! Public Squiggle parsing entry point.

use chumsky::{
    Parser,
    input::{Input, Stream},
};

use super::{
    ast::{Program, Span},
    diagnostic::Diagnostic,
    lexer::lexer,
    parser::program_parser,
    token::Token,
};

/// Parses a complete Squiggle module into a source-spanned syntax tree.
///
/// All returned spans are byte offsets into `source`. Lexical and grammatical
/// failures are accumulated when recovery permits and can be rendered with
/// [`Diagnostic::render`].
///
/// # Examples
///
/// ```
/// let program = optimist::squiggle::parse("estimate = 5 to 10\nestimate")?;
/// assert_eq!(program.statements[0].name, "estimate");
/// # Ok::<(), Vec<optimist::squiggle::Diagnostic>>(())
/// ```
pub fn parse(source: &str) -> Result<Program, Vec<Diagnostic>> {
    let (tokens, lexical_errors) = lexer().parse(source).into_output_errors();
    let mut diagnostics = lexical_errors
        .into_iter()
        .map(|error| {
            Diagnostic::syntax(
                error.to_string(),
                Span {
                    start: error.span().start,
                    end: error.span().end,
                },
            )
        })
        .collect::<Vec<_>>();
    let Some(tokens) = tokens else {
        return Err(diagnostics);
    };
    let end = (source.len()..source.len()).into();
    let stream = Stream::from_iter(tokens).map(end, |(token, span)| (token, span));
    let (program, parse_errors) = program_parser().parse(stream).into_output_errors();
    diagnostics.extend(parse_errors.into_iter().map(|error| {
        Diagnostic::syntax(
            error.to_string(),
            Span {
                start: error.span().start,
                end: error.span().end,
            },
        )
    }));
    match (program, diagnostics.is_empty()) {
        (Some(program), true) => Ok(program),
        _ => Err(diagnostics),
    }
}

/// Whether `source` uses `name` as an identifier anywhere within it.
///
/// The answer is drawn from the token stream rather than the syntax tree so that
/// it stays correct as the grammar grows: a name cannot hide inside a form the
/// lexer does not know about, and an expression too broken to parse reports the
/// names it does contain instead of none. That makes the answer an over-estimate
/// — a lambda's own parameter counts as a use of the name — which suits callers
/// asking whether something *might* depend on a binding.
pub(crate) fn names(source: &str, name: &str) -> bool {
    lexer().parse(source).into_output().is_some_and(|tokens| {
        tokens
            .iter()
            .any(|(token, _)| matches!(token, Token::Identifier(found) if found == name))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::squiggle::ast::ExpressionKind;

    fn parse_test(source: &str) -> Result<Program, String> {
        parse(source).map_err(|errors| format!("{errors:?}"))
    }

    #[test]
    fn a_name_is_found_only_where_it_stands_alone() {
        assert!(names("if t > 5 then 1 else 2", "t"));
        assert!(!names("total + attempts", "t"));
        assert!(!names("\"t\"", "t"), "a string is not a name");
        assert!(!names("1 to 10", "t"), "a keyword is not a name");
    }

    #[test]
    fn parses_bindings_functions_and_precedence() -> Result<(), String> {
        let program = parse_test("x = 2\nf(y) = x + y * 3\nf(4)")?;
        assert_eq!(program.statements.len(), 2);
        assert_eq!(program.statements[1].name, "f");
        let result = program
            .result
            .ok_or_else(|| "missing program result".to_owned())?;
        assert!(matches!(result.kind, ExpressionKind::Call { .. }));
        Ok(())
    }

    #[test]
    fn parses_core_expression_forms() -> Result<(), String> {
        let source = "value :: usd/year = 5k\nif true then value -> max(3) else {|x| x}(2)";
        let program = parse_test(source)?;
        assert!(program.statements[0].unit.is_some());
        let result = program
            .result
            .ok_or_else(|| "missing program result".to_owned())?;
        assert!(matches!(result.kind, ExpressionKind::Conditional { .. }));
        Ok(())
    }

    #[test]
    fn parses_collections_and_scoped_blocks() -> Result<(), String> {
        let program = parse_test("{ answer = 40; {name: 'value', result: answer + 2} }")?;
        let result = program
            .result
            .ok_or_else(|| "missing program result".to_owned())?;
        assert!(matches!(result.kind, ExpressionKind::Block { .. }));
        Ok(())
    }

    #[test]
    fn reports_a_source_span() -> Result<(), String> {
        let errors = parse("x = [1,")
            .err()
            .ok_or_else(|| "malformed input parsed successfully".to_owned())?;
        let error = errors
            .into_iter()
            .next()
            .ok_or_else(|| "parser returned no diagnostic".to_owned())?;
        assert!(error.span.start <= error.span.end);
        assert!(error.render("x = [1,")?.contains("Error"));
        Ok(())
    }
}
