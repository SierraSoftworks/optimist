macro_rules! builtins {
    (
        context($runtime:ident, $span:ident);
        $(
            $name:tt $(| $alias:tt)*
            ( $($parameters:tt)* ) => $body:expr
        ),* $(,)?
    ) => {
        pub(super) fn handles(name: &str) -> bool {
            false $(
                || name == builtins!(@name $name)
                $(|| name == builtins!(@name $alias))*
            )*
        }

        pub(crate) fn signatures() -> Vec<crate::squiggle::lint::BuiltinSignature> {
            vec![$(
                builtins!(@metadata
                    [builtins!(@name $name) $(, builtins!(@name $alias))*];
                    $($parameters)*
                )
            ),*]
        }

        pub(super) fn call(
            $runtime: &mut Runtime,
            name: &str,
            arguments: Vec<Value>,
            $span: Span,
        ) -> Result<Value, Diagnostic> {
            let _ = &$runtime;
            let _ = &$span;
            $(
                if name == builtins!(@name $name)
                    $(|| name == builtins!(@name $alias))*
                {
                    builtins!(@invoke arguments, $body; $($parameters)*);
                }
            )*

            if handles(name) {
                let expected = [
                    $(
                        (
                            name == builtins!(@name $name)
                                $(|| name == builtins!(@name $alias))*,
                            format!(
                                "{}({})",
                                name,
                                builtins!(@parameters $($parameters)*)
                            ),
                        )
                    ),*
                ]
                .into_iter()
                .filter_map(|(matches, signature)| matches.then_some(signature))
                .collect::<Vec<_>>()
                .join(" or ");
                let received = arguments
                    .iter()
                    .map(Value::type_name)
                    .collect::<Vec<_>>()
                    .join(", ");
                Err(Diagnostic::runtime(
                    format!(
                        "no overload of '{name}' accepts ({received}); expected {expected}"
                    ),
                    $span,
                ))
            } else {
                Err(Diagnostic::runtime(
                    format!("builtin '{name}' is not implemented"),
                    $span,
                ))
            }
        }
    };

    (@name $name:ident) => { stringify!($name) };
    (@name $name:literal) => { $name };

    (@metadata [$($name:expr),+];
        $( $parameter:ident : $kind:tt, )*
        ... $variadic:ident : $variadic_kind:tt $(,)?
    ) => {
        crate::squiggle::lint::BuiltinSignature {
            names: vec![$($name),+],
            parameters: vec![$(
                crate::squiggle::lint::ParameterConstraint {
                    name: stringify!($parameter),
                    constraint: builtins!(@constraint $kind),
                }
            ),*],
            variadic: Some(crate::squiggle::lint::ParameterConstraint {
                name: stringify!($variadic),
                constraint: builtins!(@constraint $variadic_kind),
            }),
        }
    };
    (@metadata [$($name:expr),+];
        $( $parameter:ident : $kind:tt ),* $(,)?
    ) => {
        crate::squiggle::lint::BuiltinSignature {
            names: vec![$($name),+],
            parameters: vec![$(
                crate::squiggle::lint::ParameterConstraint {
                    name: stringify!($parameter),
                    constraint: builtins!(@constraint $kind),
                }
            ),*],
            variadic: None,
        }
    };

    (@constraint *) => { crate::squiggle::lint::Constraint::Any };
    (@constraint Number) => { crate::squiggle::lint::Constraint::Number };
    (@constraint Integer) => { crate::squiggle::lint::Constraint::Integer };
    (@constraint NonNegativeInteger) => { crate::squiggle::lint::Constraint::NonNegativeInteger };
    (@constraint Boolean) => { crate::squiggle::lint::Constraint::Boolean };
    (@constraint String) => { crate::squiggle::lint::Constraint::String };
    (@constraint Array) => { crate::squiggle::lint::Constraint::Array(Box::new(crate::squiggle::lint::Constraint::Any)) };
    (@constraint Dictionary) => { crate::squiggle::lint::Constraint::Dictionary };
    (@constraint Distribution) => { crate::squiggle::lint::Constraint::Distribution };
    (@constraint Function) => { crate::squiggle::lint::Constraint::Function };
    (@constraint Date) => { crate::squiggle::lint::Constraint::Date };
    (@constraint Duration) => { crate::squiggle::lint::Constraint::Duration };
    (@constraint Domain) => { crate::squiggle::lint::Constraint::Domain };
    (@constraint [$kind:tt]) => { crate::squiggle::lint::Constraint::Array(Box::new(builtins!(@constraint $kind))) };
    (@constraint ($($kind:tt)|+)) => { crate::squiggle::lint::Constraint::Union(vec![$(builtins!(@constraint $kind)),+]) };

    (@invoke $arguments:ident, $body:expr;
        $( $parameter:ident : $kind:tt, )*
        ... $variadic:ident : $variadic_kind:tt $(,)?
    ) => {{
        let fixed_count = 0usize $(+ { let _ = stringify!($parameter); 1usize })*;
        if $arguments.len() >= fixed_count {
            let (fixed, variadic_values) = $arguments.split_at(fixed_count);
            match fixed {
                [$(builtins!(@pattern $parameter : $kind)),*]
                    if true
                        $(&& builtins!(@guard $parameter : $kind))*
                        && builtins!(@variadic_guard variadic_values : $variadic_kind) =>
                {
                    $(builtins!(@bind $parameter : $kind);)*
                    builtins!(@bind_variadic $variadic = variadic_values : $variadic_kind);
                    return $body;
                }
                _ => {}
            }
        }
    }};
    (@invoke $arguments:ident, $body:expr;
        $( $parameter:ident : $kind:tt ),* $(,)?
    ) => {{
        match $arguments.as_slice() {
            [$(builtins!(@pattern $parameter : $kind)),*]
                if true $(&& builtins!(@guard $parameter : $kind))* =>
            {
                $(builtins!(@bind $parameter : $kind);)*
                return $body;
            }
            _ => {}
        }
    }};

    (@pattern $parameter:ident : *) => { $parameter };
    (@pattern $parameter:ident : Number) => { Value::Number($parameter) };
    (@pattern $parameter:ident : Integer) => { Value::Number($parameter) };
    (@pattern $parameter:ident : NonNegativeInteger) => { Value::Number($parameter) };
    (@pattern $parameter:ident : Boolean) => { Value::Boolean($parameter) };
    (@pattern $parameter:ident : String) => { Value::String($parameter) };
    (@pattern $parameter:ident : Array) => { Value::Array($parameter) };
    (@pattern $parameter:ident : Dictionary) => { Value::Dictionary($parameter) };
    (@pattern $parameter:ident : Distribution) => { Value::Distribution($parameter) };
    (@pattern $parameter:ident : Function) => { Value::Function($parameter) };
    (@pattern $parameter:ident : Date) => { Value::Date($parameter) };
    (@pattern $parameter:ident : Duration) => { Value::Duration($parameter) };
    (@pattern $parameter:ident : Domain) => { Value::Domain($parameter) };
    (@pattern $parameter:ident : [$kind:tt]) => { Value::Array($parameter) };
    (@pattern $parameter:ident : ($($kind:tt)|+)) => { $parameter };

    (@bind $parameter:ident : *) => {};
    (@bind $parameter:ident : Number) => { let $parameter = *$parameter; };
    (@bind $parameter:ident : Integer) => { let $parameter = *$parameter as i64; };
    (@bind $parameter:ident : NonNegativeInteger) => { let $parameter = *$parameter as u64; };
    (@bind $parameter:ident : Boolean) => { let $parameter = *$parameter; };
    (@bind $parameter:ident : String) => {};
    (@bind $parameter:ident : Array) => {};
    (@bind $parameter:ident : Dictionary) => {};
    (@bind $parameter:ident : Distribution) => {};
    (@bind $parameter:ident : Function) => {};
    (@bind $parameter:ident : Date) => { let $parameter = *$parameter; };
    (@bind $parameter:ident : Duration) => { let $parameter = *$parameter; };
    (@bind $parameter:ident : Domain) => {};
    (@bind $parameter:ident : [$kind:tt]) => {
        let $parameter = builtins!(@bind_array $parameter : $kind);
    };
    (@bind $parameter:ident : ($($kind:tt)|+)) => {};

    (@guard $parameter:ident : *) => { true };
    (@guard $parameter:ident : Number) => { true };
    (@guard $parameter:ident : Integer) => {
        $parameter.is_finite()
            && $parameter.fract() == 0.0
            && *$parameter >= i64::MIN as f64
            && *$parameter <= i64::MAX as f64
    };
    (@guard $parameter:ident : NonNegativeInteger) => {
        $parameter.is_finite()
            && $parameter.fract() == 0.0
            && *$parameter >= 0.0
            && *$parameter <= u64::MAX as f64
    };
    (@guard $parameter:ident : Boolean) => { true };
    (@guard $parameter:ident : String) => { true };
    (@guard $parameter:ident : Array) => { true };
    (@guard $parameter:ident : Dictionary) => { true };
    (@guard $parameter:ident : Distribution) => { true };
    (@guard $parameter:ident : Function) => { true };
    (@guard $parameter:ident : Date) => { true };
    (@guard $parameter:ident : Duration) => { true };
    (@guard $parameter:ident : Domain) => { true };
    (@guard $parameter:ident : [$kind:tt]) => {
        $parameter.iter().all(|value| builtins!(@value_guard value : $kind))
    };
    (@guard $parameter:ident : ($($kind:tt)|+)) => {
        false $(|| builtins!(@value_guard $parameter : $kind))+
    };

    (@variadic_guard $values:ident : *) => { true };
    (@variadic_guard $values:ident : Number) => {
        $values.iter().all(|value| matches!(value, Value::Number(_)))
    };
    (@variadic_guard $values:ident : Integer) => {
        $values.iter().all(|value| match value {
            Value::Number(value) => value.is_finite()
                && value.fract() == 0.0
                && *value >= i64::MIN as f64
                && *value <= i64::MAX as f64,
            _ => false,
        })
    };
    (@variadic_guard $values:ident : NonNegativeInteger) => {
        $values.iter().all(|value| match value {
            Value::Number(value) => value.is_finite()
                && value.fract() == 0.0
                && *value >= 0.0
                && *value <= u64::MAX as f64,
            _ => false,
        })
    };
    (@variadic_guard $values:ident : Boolean) => {
        $values.iter().all(|value| matches!(value, Value::Boolean(_)))
    };
    (@variadic_guard $values:ident : String) => {
        $values.iter().all(|value| matches!(value, Value::String(_)))
    };
    (@variadic_guard $values:ident : Array) => {
        $values.iter().all(|value| matches!(value, Value::Array(_)))
    };
    (@variadic_guard $values:ident : Dictionary) => {
        $values.iter().all(|value| matches!(value, Value::Dictionary(_)))
    };
    (@variadic_guard $values:ident : Distribution) => {
        $values.iter().all(|value| matches!(value, Value::Distribution(_)))
    };
    (@variadic_guard $values:ident : Function) => {
        $values.iter().all(|value| matches!(value, Value::Function(_)))
    };
    (@variadic_guard $values:ident : Date) => {
        $values.iter().all(|value| matches!(value, Value::Date(_)))
    };
    (@variadic_guard $values:ident : Duration) => {
        $values.iter().all(|value| matches!(value, Value::Duration(_)))
    };
    (@variadic_guard $values:ident : Domain) => {
        $values.iter().all(|value| matches!(value, Value::Domain(_)))
    };
    (@variadic_guard $values:ident : [$kind:tt]) => {
        $values.iter().all(|value| builtins!(@value_guard value : [$kind]))
    };
    (@variadic_guard $values:ident : ($($kind:tt)|+)) => {
        $values.iter().all(|value| false $(|| builtins!(@value_guard value : $kind))+)
    };

    (@bind_variadic $parameter:ident = $values:ident : *) => {
        let $parameter = $values;
    };
    (@bind_variadic $parameter:ident = $values:ident : Number) => {
        let $parameter = $values.iter().filter_map(Value::as_number).collect::<Vec<_>>();
    };
    (@bind_variadic $parameter:ident = $values:ident : Integer) => {
        let $parameter = $values.iter().filter_map(Value::as_number)
            .map(|value| value as i64).collect::<Vec<_>>();
    };
    (@bind_variadic $parameter:ident = $values:ident : NonNegativeInteger) => {
        let $parameter = $values.iter().filter_map(Value::as_number)
            .map(|value| value as u64).collect::<Vec<_>>();
    };
    (@bind_variadic $parameter:ident = $values:ident : Boolean) => {
        let $parameter = $values.iter().filter_map(|value| match value {
            Value::Boolean(value) => Some(*value), _ => None,
        }).collect::<Vec<_>>();
    };
    (@bind_variadic $parameter:ident = $values:ident : String) => {
        let $parameter = $values.iter().filter_map(|value| match value {
            Value::String(value) => Some(value), _ => None,
        }).collect::<Vec<_>>();
    };
    (@bind_variadic $parameter:ident = $values:ident : Array) => {
        let $parameter = $values.iter().filter_map(|value| match value {
            Value::Array(value) => Some(value), _ => None,
        }).collect::<Vec<_>>();
    };
    (@bind_variadic $parameter:ident = $values:ident : Dictionary) => {
        let $parameter = $values.iter().filter_map(|value| match value {
            Value::Dictionary(value) => Some(value), _ => None,
        }).collect::<Vec<_>>();
    };
    (@bind_variadic $parameter:ident = $values:ident : Distribution) => {
        let $parameter = $values.iter().filter_map(Value::as_distribution).collect::<Vec<_>>();
    };
    (@bind_variadic $parameter:ident = $values:ident : Function) => {
        let $parameter = $values.iter().filter_map(|value| match value {
            Value::Function(value) => Some(value), _ => None,
        }).collect::<Vec<_>>();
    };
    (@bind_variadic $parameter:ident = $values:ident : Date) => {
        let $parameter = $values.iter().filter_map(|value| match value {
            Value::Date(value) => Some(*value), _ => None,
        }).collect::<Vec<_>>();
    };
    (@bind_variadic $parameter:ident = $values:ident : Duration) => {
        let $parameter = $values.iter().filter_map(|value| match value {
            Value::Duration(value) => Some(*value), _ => None,
        }).collect::<Vec<_>>();
    };
    (@bind_variadic $parameter:ident = $values:ident : Domain) => {
        let $parameter = $values.iter().filter_map(|value| match value {
            Value::Domain(value) => Some(value), _ => None,
        }).collect::<Vec<_>>();
    };
    (@bind_variadic $parameter:ident = $values:ident : [$kind:tt]) => {
        let $parameter = $values.iter().filter_map(|value| match value {
            Value::Array(values) => Some(builtins!(@bind_array values : $kind)),
            _ => None,
        }).collect::<Vec<_>>();
    };
    (@bind_variadic $parameter:ident = $values:ident : ($($kind:tt)|+)) => {
        let $parameter = $values;
    };

    (@value_guard $value:ident : *) => { true };
    (@value_guard $value:ident : Number) => { matches!($value, Value::Number(_)) };
    (@value_guard $value:ident : Integer) => { match $value {
        Value::Number(value) => value.is_finite()
            && value.fract() == 0.0
            && *value >= i64::MIN as f64
            && *value <= i64::MAX as f64,
        _ => false,
    }};
    (@value_guard $value:ident : NonNegativeInteger) => { match $value {
        Value::Number(value) => value.is_finite()
            && value.fract() == 0.0
            && *value >= 0.0
            && *value <= u64::MAX as f64,
        _ => false,
    }};
    (@value_guard $value:ident : Boolean) => { matches!($value, Value::Boolean(_)) };
    (@value_guard $value:ident : String) => { matches!($value, Value::String(_)) };
    (@value_guard $value:ident : Array) => { matches!($value, Value::Array(_)) };
    (@value_guard $value:ident : Dictionary) => { matches!($value, Value::Dictionary(_)) };
    (@value_guard $value:ident : Distribution) => { matches!($value, Value::Distribution(_)) };
    (@value_guard $value:ident : Function) => { matches!($value, Value::Function(_)) };
    (@value_guard $value:ident : Date) => { matches!($value, Value::Date(_)) };
    (@value_guard $value:ident : Duration) => { matches!($value, Value::Duration(_)) };
    (@value_guard $value:ident : Domain) => { matches!($value, Value::Domain(_)) };
    (@value_guard $value:ident : [$kind:tt]) => { match $value {
        Value::Array(values) => values.iter().all(|value| builtins!(@value_guard value : $kind)),
        _ => false,
    }};

    (@bind_array $values:ident : *) => { $values };
    (@bind_array $values:ident : Number) => {
        $values.iter().filter_map(Value::as_number).collect::<Vec<_>>()
    };
    (@bind_array $values:ident : Integer) => {
        $values.iter().filter_map(Value::as_number).map(|value| value as i64).collect::<Vec<_>>()
    };
    (@bind_array $values:ident : NonNegativeInteger) => {
        $values.iter().filter_map(Value::as_number).map(|value| value as u64).collect::<Vec<_>>()
    };
    (@bind_array $values:ident : Boolean) => {
        $values.iter().filter_map(|value| match value { Value::Boolean(value) => Some(*value), _ => None }).collect::<Vec<_>>()
    };
    (@bind_array $values:ident : String) => {
        $values.iter().filter_map(|value| match value { Value::String(value) => Some(value), _ => None }).collect::<Vec<_>>()
    };
    (@bind_array $values:ident : Array) => {
        $values.iter().filter_map(|value| match value { Value::Array(value) => Some(value), _ => None }).collect::<Vec<_>>()
    };
    (@bind_array $values:ident : Dictionary) => {
        $values.iter().filter_map(|value| match value { Value::Dictionary(value) => Some(value), _ => None }).collect::<Vec<_>>()
    };
    (@bind_array $values:ident : Distribution) => {
        $values.iter().filter_map(Value::as_distribution).collect::<Vec<_>>()
    };
    (@bind_array $values:ident : Function) => {
        $values.iter().filter_map(|value| match value { Value::Function(value) => Some(value), _ => None }).collect::<Vec<_>>()
    };
    (@bind_array $values:ident : Date) => {
        $values.iter().filter_map(|value| match value { Value::Date(value) => Some(*value), _ => None }).collect::<Vec<_>>()
    };
    (@bind_array $values:ident : Duration) => {
        $values.iter().filter_map(|value| match value { Value::Duration(value) => Some(*value), _ => None }).collect::<Vec<_>>()
    };
    (@bind_array $values:ident : Domain) => {
        $values.iter().filter_map(|value| match value { Value::Domain(value) => Some(value), _ => None }).collect::<Vec<_>>()
    };

    (@signature $parameter:ident : $kind:tt) => {
        concat!(stringify!($parameter), ": ", stringify!($kind))
    };
    (@parameters $( $parameter:ident : $kind:tt, )* ... $variadic:ident : $variadic_kind:tt $(,)?) => {{
        let parameters: &[&str] = &[
            $(builtins!(@signature $parameter : $kind),)*
            concat!("...", stringify!($variadic), ": ", stringify!($variadic_kind)),
        ];
        parameters.join(", ")
    }};
    (@parameters $( $parameter:ident : $kind:tt ),* $(,)?) => {{
        let parameters: &[&str] = &[$(builtins!(@signature $parameter : $kind)),*];
        parameters.join(", ")
    }};
}
