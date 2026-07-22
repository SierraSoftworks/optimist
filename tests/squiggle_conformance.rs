//! Parameterized compatibility cases derived from Squiggle's upstream tests.
//!
//! Sources:
//! - `packages/squiggle-lang/__tests__/ast/parse_test.ts`
//! - `packages/squiggle-lang/__tests__/reducer/{various,functionAssignment}_test.ts`
//! - `packages/squiggle-lang/__tests__/library/{list,dict,number,sym}_test.ts`
//!
//! UI-only values and upstream features not exposed by this sidecar are deliberately
//! excluded. Cases assert semantic values rather than upstream's presentation strings.

use optimist::squiggle::{Runtime, Value, parse};
use rstest::rstest;

fn evaluate(source: &str) -> Result<Value, String> {
    Runtime::new()
        .evaluate(source)
        .map_err(|diagnostics| format!("{diagnostics:?}"))
}

#[rstest]
#[case::integer("1", "1")]
#[case::nested_parentheses("(((1)))", "1")]
#[case::add("1+2", "3")]
#[case::precedence("1 * 2 + 3 * 4", "14")]
#[case::parenthesized_precedence("(1+2)*3", "9")]
#[case::right_associative_power("2^3^2", "512")]
#[case::unary("1 + -1", "0")]
#[case::comparison("2>1", "true")]
#[case::logical("true && !false", "true")]
#[case::ternary("false ? 2 : 5", "5")]
#[case::if_then_else("if false then 2 else 3", "3")]
#[case::assignment("x=1; y=x+1; y+1", "3")]
#[case::nested_blocks("x = { y = { z = 5; z * 2 }; y + 3 }; x", "13")]
#[case::lexical_capture("x=5; f(y)=x*y; x=6; f(2)", "10")]
#[case::function_call("f(x)=2*x; f(4)", "8")]
#[case::pipe("f(x,y)=x+y; 1->f(2)", "3")]
#[case::pipe_to_lambda("6->{|x,y| x/y}(2)", "3")]
#[case::array_lookup("([0,1,2])[1]", "1")]
#[case::dict_lookup("r={a:1}; r.a", "1")]
#[case::dict_shorthand("a=1; {a,b:a}", "{a: 1, b: 1}")]
#[case::percent_suffix("100%", "1")]
#[case::magnitude_suffix("2k+3", "2003")]
#[case::string_concat("concat('a','b','c')", "abc")]
#[case::list_make("List.make(3, {|index| index+1})", "[1, 2, 3]")]
#[case::list_map("List.map([10,20,30], {|x,i|x+i+1})", "[11, 22, 33]")]
#[case::list_reduce("List.reduce([1,2,3], 0, add)", "6")]
#[case::list_reverse("List.reverse([3,5,8])", "[8, 5, 3]")]
#[case::list_concat("List.concat([1,2], [3])", "[1, 2, 3]")]
#[case::list_filter("List.filter([1,2,3], {|x|x>1})", "[2, 3]")]
#[case::list_zip("List.zip([1,2], [3,4])", "[[1, 3], [2, 4]]")]
#[case::list_unzip("List.unzip([[1,3],[2,4]])", "[[1, 2], [3, 4]]")]
#[case::list_unique("List.uniq([1,2,1,3,2])", "[1, 2, 3]")]
#[case::list_append("List.append([3,5],8)", "[3, 5, 8]")]
#[case::list_slice("List.slice([1,2,3,4,5,6],2,4)", "[3, 4]")]
#[case::list_flatten("List.flatten([[1,2],[3,[4,5]]])", "[1, 2, 3, [4, 5]]")]
#[case::list_join("List.join(['a','b','c'],'-')", "a-b-c")]
#[case::list_sort_by("List.sortBy([5,2,3,1,4],{|n|n})", "[1, 2, 3, 4, 5]")]
#[case::list_min_by("List.minBy([5,2,3,1,4],{|n|n})", "1")]
#[case::list_max_by("List.maxBy([5,2,3,1,4],{|n|n})", "5")]
#[case::list_reduce_reverse("List.reduceReverse([1,2,3],0,{|acc,x|acc*x+x})", "9")]
#[case::list_reduce_while("List.reduceWhile([5,6,7],0,{|acc,x|acc+x},{|acc|acc<12})", "11")]
#[case::list_every("List.every([2,4,6],{|x|x>1})", "true")]
#[case::list_some("List.some([2,4,5],{|x|x>7})", "false")]
#[case::list_find("List.find([2,4,6],{|x|x>2})", "4")]
#[case::list_find_index("List.findIndex([2,4,6],{|x|x>2})", "1")]
#[case::dict_merge("Dict.merge({a:1,b:2},{b:3,c:4})", "{a: 1, b: 3, c: 4}")]
#[case::dict_set("Dict.set({a:1,b:2},'c',3)", "{a: 1, b: 2, c: 3}")]
#[case::dict_merge_many("Dict.mergeMany([{a:1},{b:2},{a:3}])", "{a: 3, b: 2}")]
#[case::dict_keys("Dict.keys({a:1,b:2})", "[a, b]")]
#[case::dict_values("Dict.values({a:1,b:2})", "[1, 2]")]
#[case::dict_to_list("Dict.toList({a:1,b:2})", "[[a, 1], [b, 2]]")]
#[case::dict_from_list("Dict.fromList([['a',1],['b',2]])", "{a: 1, b: 2}")]
#[case::dict_map("Dict.map({a:1,b:2},{|x|x*2})", "{a: 2, b: 4}")]
#[case::dict_map_keys("Dict.mapKeys({a:1,b:2},{|x|concat(x,'hi')})", "{ahi: 1, bhi: 2}")]
#[case::dict_pick("Dict.pick({a:1,b:2,c:3},['a','c'])", "{a: 1, c: 3}")]
#[case::dict_omit("Dict.omit({a:1,b:2,c:3},['a','c'])", "{b: 2}")]
#[case::duration("Duration.toHours(90minutes)", "1.5")]
#[case::date_arithmetic(
    "Date.toUnixTime(Date.make('2020-01-01')+1days)-Date.toUnixTime(Date.make('2020-01-01'))",
    "86400"
)]
#[case::string_split("String.split('a,b,c',',')", "[a, b, c]")]
#[case::sample_set_mean("mean(SampleSet.fromList([1,2,3]))", "2")]
#[case::point_set_number("mean(PointSet.make(3))", "3")]
#[case::mixture_points("mean(mixture(1,1,1))", "1")]
fn upstream_value_cases(#[case] source: &str, #[case] expected: &str) -> Result<(), String> {
    assert_eq!(evaluate(source)?.to_string(), expected);
    Ok(())
}

#[rstest]
#[case::natural_log("log(10)", 10.0_f64.ln(), 1e-12)]
#[case::cosine("Math.cos(10)", 10.0_f64.cos(), 1e-12)]
#[case::log_base("log(8,2)", 3.0, 1e-12)]
#[case::normal_mean("mean(Sym.normal(5,2))", 5.0, 1e-12)]
#[case::normal_stdev("stdev(Sym.normal(5,2))", 2.0, 1e-12)]
#[case::lognormal_mean("mean(Sym.lognormal(1,2))", 1.0_f64.exp() * 2.0_f64.exp(), 1e-10)]
#[case::gamma_mean("mean(Sym.gamma(5,5))", 25.0, 1e-12)]
#[case::gamma_stdev("stdev(Sym.gamma(5,5))", 125.0_f64.sqrt(), 1e-12)]
#[case::bernoulli_mean("mean(Sym.bernoulli(0.2))", 0.2, 1e-12)]
#[case::logistic_stdev("stdev(Sym.logistic(5,1))", std::f64::consts::PI / 3.0_f64.sqrt(), 1e-12)]
#[case::normal_ci_low("quantile(Sym.normal({p5:-2,p95:4}),0.05)", -2.0, 1e-8)]
#[case::normal_ci_high("quantile(Sym.normal({p5:-2,p95:4}),0.95)", 4.0, 1e-8)]
#[case::lognormal_ci_low("quantile(Sym.lognormal({p10:2,p90:5}),0.1)", 2.0, 1e-8)]
#[case::lognormal_ci_high("quantile(Sym.lognormal({p10:2,p90:5}),0.9)", 5.0, 1e-8)]
fn upstream_numeric_cases(
    #[case] source: &str,
    #[case] expected: f64,
    #[case] tolerance: f64,
) -> Result<(), String> {
    let received = evaluate(source)?
        .as_number()
        .ok_or_else(|| format!("{source} did not return a Number"))?;
    assert!(
        (received - expected).abs() <= tolerance,
        "{source}: expected {expected}, received {received}, tolerance {tolerance}"
    );
    Ok(())
}

#[rstest]
#[case::unknown_identifier("missing(1)", "unknown identifier")]
#[case::fractional_index("[3,5,8][1.8]", "cannot index Array with Number")]
#[case::missing_index("[3,5,8][10]", "array index is out of bounds")]
#[case::missing_dict_key("{a:1}.b", "dictionary has no key 'b'")]
#[case::list_first_empty("List.first([])", "list must not be empty")]
#[case::bad_list_make_count("List.make(3.5,'x')", "NonNegativeInteger")]
#[case::bad_nested_numbers("mean([1,'two'])", "[Number]")]
#[case::bad_parameter_domain("f(x:Number.rangeDomain(0,1))=x; f(2)", "outside its declared domain")]
#[case::bad_distribution("normal(0,0)", "standard deviation must be greater than zero")]
#[case::bad_variadic_element("concat('a',2)", "...values: String")]
#[case::bad_dict_entries("Dict.fromList([['a',1],[2,3]])", "[String, value]")]
#[case::bad_sample_set_values("SampleSet.fromList([1,'two'])", "[Number]")]
#[case::bad_list_callback("List.filter([1,2],{|x|x})", "Boolean callback result")]
#[case::bad_date("Date.make('not-a-date')", "input contains invalid characters")]
fn upstream_error_cases(#[case] source: &str, #[case] message: &str) -> Result<(), String> {
    let diagnostics = Runtime::new()
        .evaluate(source)
        .err()
        .ok_or_else(|| format!("{source} unexpectedly succeeded"))?;
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(message)),
        "{source}: expected diagnostic containing {message:?}, got {diagnostics:?}"
    );
    Ok(())
}

#[rstest]
#[case::empty("")]
#[case::comment_only("// comment")]
#[case::decimal_trailing("1.")]
#[case::decimal_leading(".001")]
#[case::scientific("0.1e-3")]
#[case::unit_signature("x :: kg*m^2/s^3 = 1")]
#[case::function_units("f(x :: m, y :: s) :: m/s = x/y")]
#[case::trailing_array_comma("[3,4,]")]
#[case::trailing_dict_comma("{a:1,b:2,}")]
#[case::multiline_comment("/* first\nsecond */ 1")]
#[case::nested_ternary("false ? 2 : false ? 4 : 5")]
#[case::nested_if("if false then {2} else if false then {4} else {5}")]
#[case::logical_precedence("a && b<c[i] || d")]
#[case::equality_operator("a==b")]
#[case::pointwise_operator("normal(5,2) .+ normal(5,1)")]
#[case::pipe_newline("1\n -> add(2)")]
#[case::lambda("{|x|x}")]
#[case::zero_argument_lambda("{||1}")]
fn upstream_parse_cases(#[case] source: &str) {
    let result = parse(source);
    assert!(result.is_ok(), "failed to parse {source:?}: {result:?}");
}
