//! Registry-driven smoke coverage for every core builtin binding.

use optimist::squiggle::{Runtime, builtin_names, lint};

fn smoke_source(name: &str) -> Option<String> {
    let call = match name {
        "typeOf" => "typeOf(1)",
        "inspect" => "inspect(1)",
        "throw" => "try({||throw()},{||1}).value",
        "try" => "try({||1},{||2}).value",
        "String.make" => "String.make(1)",
        "String.split" => "String.split('a,b',',')",
        "System.sampleCount" => "System.sampleCount()",
        "Little.occupancy" => "Little.occupancy(100,0.25)",
        "Little.residence" => "Little.residence(25,100)",
        "Little.rate" => "Little.rate(25,0.25)",
        "Queue.utilisation" => "Queue.utilisation(80,100)",
        "Queue.utilization" => "Queue.utilization(80,100)",
        "Queue.mm1Wait" => "Queue.mm1Wait(0.01,0.8)",
        "Queue.mmcWait" => "Queue.mmcWait(0.01,4,0.8)",
        "Queue.erlangB" => "Queue.erlangB(4,3)",
        "Queue.erlangC" => "Queue.erlangC(4,3)",
        "Queue.boundedLength" => "Queue.boundedLength(0.8,100)",
        "Queue.boundedBlocking" => "Queue.boundedBlocking(0.8,100)",
        "Reliability.retrySuccess" => "Reliability.retrySuccess(0.9,3)",
        "Reliability.retryAttempts" => "Reliability.retryAttempts(0.9,3)",
        "Reliability.serialSuccess" => "Reliability.serialSuccess(0.99,8)",
        "Reliability.deadlineSuccess" => "Reliability.deadlineSuccess(4,0.05,1)",
        "Slo.errorBudget" => "Slo.errorBudget(1000,0.999,3600)",
        "Slo.burnRate" => "Slo.burnRate(0.002,0.999)",
        "Number.rangeDomain" => "Number.rangeDomain(0,1)",
        "Date.make" => "Date.make('2020-01-01')",
        "Date.fromUnixTime" => "Date.fromUnixTime(0)",
        "Date.toUnixTime" => "Date.toUnixTime(Date.make('2020-01-01'))",
        "Date.rangeDomain" => "Date.rangeDomain(Date.make(2020),Date.make(2021))",
        "Duration.fromMinutes" | "fromMinutes" => "fromMinutes(1)",
        "Duration.fromHours" | "fromHours" => "fromHours(1)",
        "Duration.fromDays" | "fromDays" => "fromDays(1)",
        "Duration.fromYears" | "fromYears" => "fromYears(1)",
        "Duration.toMinutes" | "toMinutes" => "toMinutes(1hours)",
        "Duration.toHours" | "toHours" => "toHours(1days)",
        "Duration.toDays" | "toDays" => "toDays(1years)",
        "Duration.toYears" | "toYears" => "toYears(365.25days)",
        "List.length" => "List.length([1])",
        "List.first" => "List.first([1])",
        "List.last" => "List.last([1])",
        "List.reverse" => "List.reverse([1,2])",
        "List.concat" => "List.concat([1],[2])",
        "List.append" => "List.append([1],2)",
        "List.slice" => "List.slice([1,2],0,1)",
        "List.upTo" => "List.upTo(1,2)",
        "List.map" => "List.map([1],{|x|x})",
        "List.reduce" => "List.reduce([1],0,add)",
        "List.filter" => "List.filter([1],{|x|true})",
        "List.make" => "List.make(1,1)",
        "List.every" => "List.every([1],{|x|true})",
        "List.some" => "List.some([1],{|x|true})",
        "List.find" => "List.find([1],{|x|true})",
        "List.findIndex" => "List.findIndex([1],{|x|true})",
        "List.flatten" => "List.flatten([[1]])",
        "List.join" => "List.join(['a'],',')",
        "List.zip" => "List.zip([1],[2])",
        "List.unzip" => "List.unzip([[1,2]])",
        "List.uniq" => "List.uniq([1,1])",
        "List.sortBy" => "List.sortBy([1],{|x|x})",
        "List.minBy" => "List.minBy([1],{|x|x})",
        "List.maxBy" => "List.maxBy([1],{|x|x})",
        "List.reduceReverse" => "List.reduceReverse([1],0,add)",
        "List.reduceWhile" => "List.reduceWhile([1],0,add,{|x|true})",
        "List.shuffle" => "List.shuffle([1])",
        "List.sample" => "List.sample([1])",
        "List.sampleN" => "List.sampleN([1],1)",
        "Dict.set" => "Dict.set({a:1},'b',2)",
        "Dict.has" => "Dict.has({a:1},'a')",
        "Dict.size" => "Dict.size({a:1})",
        "Dict.delete" => "Dict.delete({a:1},'a')",
        "Dict.merge" => "Dict.merge({a:1},{b:2})",
        "Dict.keys" => "Dict.keys({a:1})",
        "Dict.values" => "Dict.values({a:1})",
        "Dict.map" => "Dict.map({a:1},{|x|x})",
        "Dict.fromList" => "Dict.fromList([['a',1]])",
        "Dict.toList" => "Dict.toList({a:1})",
        "Dict.mergeMany" => "Dict.mergeMany([{a:1}])",
        "Dict.mapKeys" => "Dict.mapKeys({a:1},{|x|x})",
        "Dict.pick" => "Dict.pick({a:1},['a'])",
        "Dict.omit" => "Dict.omit({a:1},['a'])",
        "SampleSet" | "SampleSet.make" => "SampleSet.make(1)",
        "SampleSet.fromDist" => "SampleSet.fromDist(normal(0,1))",
        "SampleSet.fromNumber" => "SampleSet.fromNumber(1)",
        "SampleSet.fromList" => "SampleSet.fromList([1,2])",
        "SampleSet.fromFn" => "SampleSet.fromFn({||1})",
        "SampleSet.toList" => "SampleSet.toList(SampleSet.fromList([1]))",
        "SampleSet.map" => "SampleSet.map(SampleSet.fromList([1]),{|x|x})",
        "SampleSet.map2" => {
            "SampleSet.map2(SampleSet.fromList([1]),SampleSet.fromList([2]),{|x,y|x+y})"
        }
        "SampleSet.map3" => {
            "SampleSet.map3(SampleSet.fromList([1]),SampleSet.fromList([2]),SampleSet.fromList([3]),{|x,y,z|x+y+z})"
        }
        "PointSet" | "PointSet.make" => "PointSet.make(1)",
        "PointSet.fromDist" => "PointSet.fromDist(normal(0,1))",
        "PointSet.fromNumber" => "PointSet.fromNumber(1)",
        "PointSet.downsample" => "PointSet.downsample(normal(0,1),10)",
        "PointSet.support" => "PointSet.support(uniform(0,1))",
        "Dist.make" => "Dist.make(1)",
        "Dist.klDivergence" => "Dist.klDivergence(Sym.normal(0,1),Sym.normal(0,1))",
        "Dist.logScore" => "Dist.logScore({estimate:Sym.normal(0,1),answer:0})",
        "add" => "add(1,2)",
        "subtract" => "subtract(2,1)",
        "multiply" => "multiply(2,3)",
        "divide" => "divide(6,2)",
        "pow" => "pow(2,3)",
        "equal" => "equal(1,1)",
        "unequal" => "unequal(1,2)",
        "smaller" => "smaller(1,2)",
        "smallerEq" => "smallerEq(1,1)",
        "larger" => "larger(2,1)",
        "largerEq" => "largerEq(1,1)",
        "and" => "and(true,true)",
        "or" => "or(true,false)",
        "not" => "not(false)",
        "unaryMinus" => "unaryMinus(1)",
        "concat" => "concat('a','b')",
        "exp" => "exp(1)",
        "log" => "log(2)",
        "log10" => "log10(10)",
        "log2" => "log2(2)",
        "floor" => "floor(1.5)",
        "ceil" => "ceil(1.5)",
        "abs" => "abs(-1)",
        "round" => "round(1.5)",
        "mod" => "mod(5,2)",
        "sqrt" | "Math.sqrt" => "sqrt(4)",
        "sin" | "Math.sin" => "sin(0)",
        "cos" | "Math.cos" => "cos(0)",
        "tan" | "Math.tan" => "tan(0)",
        "asin" | "Math.asin" => "asin(0)",
        "acos" | "Math.acos" => "acos(0)",
        "atan" | "Math.atan" => "atan(0)",
        "normal" | "Dist.normal" | "Sym.normal" => "normal(0,1)",
        "lognormal" | "Dist.lognormal" | "Sym.lognormal" => "lognormal(0,1)",
        "uniform" | "Dist.uniform" | "Sym.uniform" => "uniform(0,1)",
        "beta" | "Dist.beta" | "Sym.beta" => "beta(2,2)",
        "cauchy" | "Sym.cauchy" => "cauchy(0,1)",
        "gamma" | "Dist.gamma" | "Sym.gamma" => "gamma(2,1)",
        "logistic" | "Sym.logistic" => "logistic(0,1)",
        "exponential" | "Sym.exponential" => "exponential(1)",
        "bernoulli" | "Sym.bernoulli" => "bernoulli(0.5)",
        "binomial" => "binomial(2,0.5)",
        "poisson" => "poisson(2)",
        "triangular" | "Sym.triangular" => "triangular(0,1,2)",
        "pointMass" => "pointMass(1)",
        "mixture" | "mx" | "Dist.mixture" => "mixture(1,2)",
        "cdf" | "Dist.cdf" => "cdf(normal(0,1),0)",
        "pdf" | "Dist.pdf" => "pdf(normal(0,1),0)",
        "inv" | "Dist.inv" => "inv(normal(0,1),0.5)",
        "sample" | "Dist.sample" => "sample(pointMass(1))",
        "sampleN" | "Dist.sampleN" => "sampleN(pointMass(1),2)",
        "truncate" => "truncate(normal(0,1),-1,1)",
        "truncateLeft" => "truncateLeft(normal(0,1),-1)",
        "truncateRight" => "truncateRight(normal(0,1),1)",
        "sum" => "sum([1,2])",
        "product" => "product([2,3])",
        "mean" => "mean([1,2])",
        "median" => "median([1,2])",
        "quantile" => "quantile([1,2],0.5)",
        "stdev" => "stdev([1,2])",
        "variance" => "variance([1,2])",
        "min" => "min([1,2])",
        "max" => "max([1,2])",
        "mode" => "mode(normal(0,1))",
        "sort" => "sort([2,1])",
        "cumsum" => "cumsum([1,2])",
        "cumprod" => "cumprod([1,2])",
        "diff" => "diff([1,2])",
        _ => return None,
    };
    Some(call.into())
}

#[test]
fn every_registered_builtin_has_a_passing_smoke_case() {
    let mut missing = Vec::new();
    let mut failures = Vec::new();
    let mut lint_failures = Vec::new();
    for name in builtin_names() {
        let Some(source) = smoke_source(name) else {
            missing.push(name);
            continue;
        };
        if let Err(diagnostics) = Runtime::new().evaluate(&source) {
            failures.push((name, source, diagnostics));
            continue;
        }
        let diagnostics = lint(&source);
        if !diagnostics.is_empty() {
            lint_failures.push((name, source, diagnostics));
        }
    }
    assert!(missing.is_empty(), "missing smoke cases for {missing:?}");
    assert!(failures.is_empty(), "builtin smoke failures: {failures:#?}");
    assert!(
        lint_failures.is_empty(),
        "builtin lint smoke failures: {lint_failures:#?}"
    );
}
