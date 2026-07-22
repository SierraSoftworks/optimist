use optimist::squiggle::{DiagnosticKind, lint};
use rstest::rstest;

#[rstest]
#[case::arithmetic("1+2*3")]
#[case::condition("if true then 1 else 2")]
#[case::builtin("normal(0,1)->mean")]
#[case::collection("List.map([1,2],{|x|x+1})")]
#[case::dictionary("Dict.set({a:1},'b',2)")]
#[case::parameter_domain("f(x:Number.rangeDomain(0,1))=x; f(0.5)")]
#[case::unit_product("distance::m=10; time::s=2; speed::m/s=distance/time; speed")]
#[case::typed_function("f(x::m,y::s)::m/s=x/y; distance::m=1; time::s=1; f(distance,time)")]
#[case::heterogeneous_data("[1,'two',true]")]
fn valid_calculations_have_no_lint_findings(#[case] source: &str) {
    assert_eq!(lint(source), Vec::new(), "{source}");
}

#[rstest]
#[case::unknown_identifier("missing+1", "unknown identifier 'missing'")]
#[case::operator_type("1+true", "operator '+' does not accept Number and Boolean")]
#[case::operator_builtin_type("add(1,true)", "operator '+' does not accept Number and Boolean")]
#[case::condition_type("if 1 then 2 else 3", "condition must be Boolean")]
#[case::builtin_type("mean('x')", "no overload of 'mean' accepts (String)")]
#[case::composite_type("mean([1,'x'])", "no overload of 'mean' accepts (Array)")]
#[case::builtin_arity("List.length([1],2)", "no overload of 'List.length'")]
#[case::array_index("[1,2]['x']", "array index must be Number")]
#[case::missing_dict_field("{a:1}.b", "dictionary has no known key 'b'")]
#[case::condition_builtin("and(true,1)", "no overload of 'and'")]
#[case::user_arity("f(x)=x; f(1,2)", "function expects 1 arguments, received 2")]
#[case::parameter_annotation(
    "f(x:Number.rangeDomain(0,1))=x; f('x')",
    "argument 1 expects Number, received String"
)]
#[case::unit_addition("x::m=1; y::s=2; x+y", "incompatible units m and s")]
#[case::unit_assignment("x::m=1; y::s=x", "declared unit s does not match inferred unit m")]
#[case::unit_return("f(x::m)::s=x", "declared unit s does not match inferred unit m")]
#[case::unit_exponent("x::m=2; y::s=2; x^y", "power exponent must be dimensionless")]
#[case::qualified_builtin("Date.toUnixTime(1)", "no overload of 'Date.toUnixTime'")]
#[case::unknown_qualified_builtin("List.missing([1])", "unknown builtin 'List.missing'")]
#[case::variadic_type("concat('a',2)", "no overload of 'concat'")]
fn invalid_calculations_report_source_spanned_lint_findings(
    #[case] source: &str,
    #[case] message: &str,
) {
    let diagnostics = lint(source);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == DiagnosticKind::Lint
                && diagnostic.message.contains(message)
                && diagnostic.span.end >= diagnostic.span.start
                && diagnostic.span.end <= source.len()
        }),
        "{source}: expected lint containing {message:?}, got {diagnostics:#?}"
    );
}

#[test]
fn syntax_errors_are_returned_by_lint() {
    let diagnostics = lint("x = [1,");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == DiagnosticKind::Syntax)
    );
}
