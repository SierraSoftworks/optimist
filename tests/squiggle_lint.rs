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

/// System-design arithmetic checks against the canonical unit vocabulary.
///
/// These are the identities a capacity model is built from: Little's Law
/// relating occupancy to a rate and a residence time, throughput against a
/// service time, storage footprint from an ingest rate and a retention window,
/// and reliability arithmetic that turns demand into outcomes. Each is written
/// the way an author would write it, so the vocabulary is exercised end to end
/// rather than only through the registry.
#[rstest]
#[case::littles_law("rate::rps=100; wait::s=0.2; occupancy::op=rate*wait; occupancy")]
#[case::service_capacity(
    "parallel::op=8; service::s=0.02; capacity::rps=parallel/service; capacity"
)]
#[case::retention("ingest::op/s=50; ttl::s=86400; stored::op=ingest*ttl; stored")]
#[case::payload_bandwidth("rate::rps=100; size::B/op=2048; demand::Bps=rate*size; demand")]
#[case::error_rate("demand::op/s=500; ratio::error/op=0.001; errors::error/s=demand*ratio; errors")]
#[case::availability("demand::op/s=500; sli::success/op=0.999; good::success/s=demand*sli; good")]
#[case::alias_agreement("a::rps=1; b::requests/second=2; a+b")]
#[case::compound_alias("a::iops=1; b::op/s=2; a+b")]
#[case::custom_quantity(
    "shards::shard=8; perShard::op/s=10; total::shard*op/s=shards*perShard; total"
)]
fn system_design_units_check(#[case] source: &str) {
    assert_eq!(lint(source), Vec::new(), "{source}");
}

/// Quantities that only look interchangeable are rejected.
///
/// Annotations are never rescaled at runtime, so a scale difference is an
/// arithmetic error rather than a notational one. Outcomes stay distinct from
/// demand, which is what makes an error ratio and a service level indicator
/// carry different units instead of both collapsing to a bare fraction.
///
/// The last two cases are the mistakes this vocabulary exists to catch. Dividing
/// a bare count by a service time yields $\text{s}^{-1}$ rather than a
/// throughput, and multiplying a request rate by a payload measured in bytes
/// yields $\text{B}\cdot\text{op}\cdot\text{s}^{-1}$ rather than bandwidth.
/// Both read as obviously correct in prose and are wrong in the algebra.
#[rstest]
#[case::scale_mismatch("a::ms=1; b::s=2; a+b", "incompatible units")]
#[case::information_scale("a::KiB=1; b::B=2; a+b", "incompatible units")]
#[case::outcome_versus_demand("good::success=1; total::op=2; good+total", "incompatible units")]
#[case::success_versus_error("good::success=1; bad::error=2; good+bad", "incompatible units")]
#[case::rate_versus_count("rate::rps=1; count::op=2; rate+count", "incompatible units")]
#[case::wrong_littles_law(
    "rate::rps=100; wait::s=0.2; occupancy::s=rate*wait; occupancy",
    "declared unit s does not match inferred unit op"
)]
#[case::sli_is_not_an_error_ratio(
    "good::sli=0.999; bad::errorRatio=0.001; good+bad",
    "incompatible units"
)]
#[case::dimensionless_capacity(
    "cores::1=8; service::s=0.02; capacity::rps=cores/service; capacity",
    "declared unit op*s^-1 does not match inferred unit s^-1"
)]
#[case::payload_missing_per_operation(
    "rate::rps=100; size::B=2048; demand::Bps=rate*size; demand",
    "declared unit B*s^-1 does not match inferred unit B*op*s^-1"
)]
fn incompatible_system_design_units_are_reported(#[case] source: &str, #[case] message: &str) {
    let diagnostics = lint(source);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == DiagnosticKind::Lint && diagnostic.message.contains(message)
        }),
        "{source}: expected lint containing {message:?}, got {diagnostics:#?}"
    );
}
