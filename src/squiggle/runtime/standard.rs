use std::collections::BTreeMap;

use crate::squiggle::{
    Value,
    value::{Environment, Function},
};

const GLOBALS: &[&str] = &[
    "add",
    "subtract",
    "multiply",
    "divide",
    "pow",
    "equal",
    "unequal",
    "smaller",
    "smallerEq",
    "larger",
    "largerEq",
    "and",
    "or",
    "not",
    "unaryMinus",
    "exp",
    "log",
    "log10",
    "log2",
    "floor",
    "ceil",
    "abs",
    "round",
    "mod",
    "sqrt",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "concat",
    "normal",
    "lognormal",
    "uniform",
    "beta",
    "cauchy",
    "gamma",
    "logistic",
    "exponential",
    "bernoulli",
    "binomial",
    "poisson",
    "triangular",
    "pointMass",
    "mixture",
    "mx",
    "cdf",
    "pdf",
    "inv",
    "sample",
    "sampleN",
    "truncate",
    "truncateLeft",
    "truncateRight",
    "sum",
    "product",
    "mean",
    "median",
    "quantile",
    "stdev",
    "variance",
    "min",
    "max",
    "mode",
    "sort",
    "cumsum",
    "cumprod",
    "diff",
    "fromMinutes",
    "fromHours",
    "fromDays",
    "fromYears",
    "toMinutes",
    "toHours",
    "toDays",
    "toYears",
    "typeOf",
    "inspect",
    "throw",
    "try",
];

const NAMESPACES: &[(&str, &[(&str, &str)])] = &[
    (
        "Math",
        &[
            ("sqrt", "Math.sqrt"),
            ("sin", "Math.sin"),
            ("cos", "Math.cos"),
            ("tan", "Math.tan"),
            ("asin", "Math.asin"),
            ("acos", "Math.acos"),
            ("atan", "Math.atan"),
        ],
    ),
    ("Number", &[("rangeDomain", "Number.rangeDomain")]),
    (
        "Dist",
        &[
            ("make", "Dist.make"),
            ("normal", "Dist.normal"),
            ("lognormal", "Dist.lognormal"),
            ("uniform", "Dist.uniform"),
            ("beta", "Dist.beta"),
            ("gamma", "Dist.gamma"),
            ("mixture", "Dist.mixture"),
            ("cdf", "Dist.cdf"),
            ("pdf", "Dist.pdf"),
            ("inv", "Dist.inv"),
            ("sample", "Dist.sample"),
            ("sampleN", "Dist.sampleN"),
            ("klDivergence", "Dist.klDivergence"),
            ("logScore", "Dist.logScore"),
        ],
    ),
    (
        "Sym",
        &[
            ("normal", "Sym.normal"),
            ("lognormal", "Sym.lognormal"),
            ("uniform", "Sym.uniform"),
            ("beta", "Sym.beta"),
            ("cauchy", "Sym.cauchy"),
            ("gamma", "Sym.gamma"),
            ("logistic", "Sym.logistic"),
            ("exponential", "Sym.exponential"),
            ("bernoulli", "Sym.bernoulli"),
            ("triangular", "Sym.triangular"),
        ],
    ),
    (
        "List",
        &[
            ("length", "List.length"),
            ("first", "List.first"),
            ("last", "List.last"),
            ("reverse", "List.reverse"),
            ("concat", "List.concat"),
            ("append", "List.append"),
            ("slice", "List.slice"),
            ("upTo", "List.upTo"),
            ("map", "List.map"),
            ("reduce", "List.reduce"),
            ("filter", "List.filter"),
            ("make", "List.make"),
            ("every", "List.every"),
            ("some", "List.some"),
            ("find", "List.find"),
            ("findIndex", "List.findIndex"),
            ("flatten", "List.flatten"),
            ("join", "List.join"),
            ("zip", "List.zip"),
            ("unzip", "List.unzip"),
            ("uniq", "List.uniq"),
            ("sortBy", "List.sortBy"),
            ("minBy", "List.minBy"),
            ("maxBy", "List.maxBy"),
            ("reduceReverse", "List.reduceReverse"),
            ("reduceWhile", "List.reduceWhile"),
            ("shuffle", "List.shuffle"),
            ("sample", "List.sample"),
            ("sampleN", "List.sampleN"),
        ],
    ),
    (
        "Dict",
        &[
            ("set", "Dict.set"),
            ("has", "Dict.has"),
            ("size", "Dict.size"),
            ("delete", "Dict.delete"),
            ("merge", "Dict.merge"),
            ("keys", "Dict.keys"),
            ("values", "Dict.values"),
            ("map", "Dict.map"),
            ("fromList", "Dict.fromList"),
            ("toList", "Dict.toList"),
            ("mergeMany", "Dict.mergeMany"),
            ("mapKeys", "Dict.mapKeys"),
            ("pick", "Dict.pick"),
            ("omit", "Dict.omit"),
        ],
    ),
    (
        "SampleSet",
        &[
            ("make", "SampleSet.make"),
            ("fromDist", "SampleSet.fromDist"),
            ("fromNumber", "SampleSet.fromNumber"),
            ("fromList", "SampleSet.fromList"),
            ("fromFn", "SampleSet.fromFn"),
            ("toList", "SampleSet.toList"),
            ("map", "SampleSet.map"),
            ("map2", "SampleSet.map2"),
            ("map3", "SampleSet.map3"),
        ],
    ),
    (
        "PointSet",
        &[
            ("make", "PointSet.make"),
            ("fromDist", "PointSet.fromDist"),
            ("fromNumber", "PointSet.fromNumber"),
            ("downsample", "PointSet.downsample"),
            ("support", "PointSet.support"),
        ],
    ),
    (
        "Date",
        &[
            ("make", "Date.make"),
            ("fromUnixTime", "Date.fromUnixTime"),
            ("toUnixTime", "Date.toUnixTime"),
            ("rangeDomain", "Date.rangeDomain"),
        ],
    ),
    (
        "Duration",
        &[
            ("fromMinutes", "Duration.fromMinutes"),
            ("fromHours", "Duration.fromHours"),
            ("fromDays", "Duration.fromDays"),
            ("fromYears", "Duration.fromYears"),
            ("toMinutes", "Duration.toMinutes"),
            ("toHours", "Duration.toHours"),
            ("toDays", "Duration.toDays"),
            ("toYears", "Duration.toYears"),
        ],
    ),
    (
        "String",
        &[("make", "String.make"), ("split", "String.split")],
    ),
    ("System", &[("sampleCount", "System.sampleCount")]),
    (
        "Little",
        &[
            ("occupancy", "Little.occupancy"),
            ("residence", "Little.residence"),
            ("rate", "Little.rate"),
        ],
    ),
    (
        "Queue",
        &[
            ("utilisation", "Queue.utilisation"),
            ("utilization", "Queue.utilization"),
            ("mm1Wait", "Queue.mm1Wait"),
            ("mmcWait", "Queue.mmcWait"),
            ("erlangB", "Queue.erlangB"),
            ("erlangC", "Queue.erlangC"),
            ("boundedLength", "Queue.boundedLength"),
            ("boundedBlocking", "Queue.boundedBlocking"),
        ],
    ),
    (
        "Reliability",
        &[
            ("retrySuccess", "Reliability.retrySuccess"),
            ("retryAttempts", "Reliability.retryAttempts"),
            ("serialSuccess", "Reliability.serialSuccess"),
            ("deadlineSuccess", "Reliability.deadlineSuccess"),
            ("quorumSuccess", "Reliability.quorumSuccess"),
            ("quorumLatency", "Reliability.quorumLatency"),
        ],
    ),
    (
        "Slo",
        &[
            ("errorBudget", "Slo.errorBudget"),
            ("burnRate", "Slo.burnRate"),
        ],
    ),
];

pub(crate) fn builtin_names() -> Vec<&'static str> {
    let mut names = GLOBALS.to_vec();
    names.extend(["SampleSet", "PointSet"]);
    names.extend(
        NAMESPACES
            .iter()
            .flat_map(|(_, functions)| functions.iter().map(|(_, qualified)| *qualified)),
    );
    names.sort_unstable();
    names.dedup();
    names
}

/// Returns the shared root scope holding every builtin binding.
///
/// The frame is built once per thread and handed out by reference, because
/// assembling a couple of hundred entries costs more than evaluating the short
/// expressions a caller re-runs thousands of times. Sharing it is sound because
/// a run never writes here: bindings and program locals live in child scopes.
pub(super) fn environment() -> Environment {
    thread_local! {
        static ROOT: Environment = build();
    }
    ROOT.with(Clone::clone)
}

fn build() -> Environment {
    let environment = Environment::root();
    environment.define("pi", Value::Number(std::f64::consts::PI));
    environment.define("e", Value::Number(std::f64::consts::E));
    environment.define("infinity", Value::Number(f64::INFINITY));
    environment.define("SampleSet", Value::Function(Function::builtin("SampleSet")));
    environment.define("PointSet", Value::Function(Function::builtin("PointSet")));
    for name in GLOBALS {
        environment.define(*name, Value::Function(Function::builtin(name)));
    }
    for (namespace, functions) in NAMESPACES {
        let values = functions
            .iter()
            .map(|(name, qualified)| {
                (
                    (*name).to_owned(),
                    Value::Function(Function::builtin(qualified)),
                )
            })
            .collect::<BTreeMap<_, _>>();
        environment.define(*namespace, Value::dictionary(values));
    }
    environment
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_registered_builtin_has_generated_signature_metadata() {
        let signatures = super::super::builtin::signatures();
        let missing = super::builtin_names()
            .into_iter()
            .filter(|name| {
                !signatures
                    .iter()
                    .any(|signature| signature.names.contains(name))
            })
            .collect::<Vec<_>>();
        assert!(
            missing.is_empty(),
            "missing builtin signatures: {missing:?}"
        );
    }
}
