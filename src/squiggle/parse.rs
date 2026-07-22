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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::squiggle::ast::ExpressionKind;

    fn parse_test(source: &str) -> Result<Program, String> {
        parse(source).map_err(|errors| format!("{errors:?}"))
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
