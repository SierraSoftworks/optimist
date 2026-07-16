pub(super) fn serialize<T: serde::Serialize + ?Sized>(
    value: &T,
) -> Result<String, human_errors::Error> {
    serde_json::to_string(value).map_err(|error| {
        human_errors::wrap_system(
            error,
            "Optimist could not serialize command output.",
            &["Retry with `--output table` and report the serialization failure if it persists."],
        )
    })
}

pub(super) fn lines<T: serde::Serialize>(values: &[T]) -> Result<String, human_errors::Error> {
    values
        .iter()
        .map(serialize)
        .collect::<Result<Vec<_>, _>>()
        .map(|lines| lines.join("\n"))
}
