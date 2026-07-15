#[path = "../fuzz_targets/json_limits.rs"]
mod json_limits;

#[test]
fn accepts_json_at_configured_shape_limits() {
    let collection = format!("[{}]", vec!["0"; 32].join(","));
    let nested = format!("{}0{}", "[".repeat(16), "]".repeat(16));
    let string = format!("\"{}\"", "a".repeat(511));

    assert!(json_limits::within_limits(collection.as_bytes()));
    assert!(json_limits::within_limits(nested.as_bytes()));
    assert!(json_limits::within_limits(string.as_bytes()));
}

#[test]
fn rejects_input_collection_recursion_and_string_overruns() {
    let oversized_input = vec![b'0'; json_limits::MAX_INPUT_BYTES + 1];
    let collection = format!("[{}]", vec!["0"; 33].join(","));
    let nested = format!("{}0{}", "[".repeat(17), "]".repeat(17));
    let string = format!("\"{}\"", "a".repeat(512));
    let escaped_string = format!("\"{}\"", "\\a".repeat(256));

    assert!(!json_limits::within_limits(&oversized_input));
    assert!(!json_limits::within_limits(collection.as_bytes()));
    assert!(!json_limits::within_limits(nested.as_bytes()));
    assert!(!json_limits::within_limits(string.as_bytes()));
    assert!(!json_limits::within_limits(escaped_string.as_bytes()));
}
