#![no_main]

mod json_limits;

use std::fmt::Debug;

use libfuzzer_sys::fuzz_target;
use optimist::domain::{Edge, EdgePayload, Node, NodePayload, Observation};
use serde::{Serialize, de::DeserializeOwned};

fuzz_target!(|data: &[u8]| {
    if !json_limits::within_limits(data) {
        return;
    }

    round_trip::<Node>(data);
    round_trip::<NodePayload>(data);
    round_trip::<Edge>(data);
    round_trip::<EdgePayload>(data);
    round_trip::<Observation>(data);
});

fn round_trip<T>(data: &[u8])
where
    T: Debug + DeserializeOwned + PartialEq + Serialize,
{
    let Ok(value) = serde_json::from_slice::<T>(data) else {
        return;
    };
    let encoded = serde_json::to_vec(&value).expect("decoded aggregate must serialize");
    let decoded = serde_json::from_slice(&encoded).expect("serialized aggregate must decode");
    assert_eq!(value, decoded);
}
