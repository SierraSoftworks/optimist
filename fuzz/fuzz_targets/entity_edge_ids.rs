#![no_main]

use std::str::FromStr;

use libfuzzer_sys::fuzz_target;
use optimist::domain::{EdgeId, EntityId};

const MAX_ID_INPUT_BYTES: usize = 128;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_ID_INPUT_BYTES {
        return;
    }
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    if let Ok(id) = EntityId::from_str(text) {
        assert_eq!(id.to_string(), text);
    }
    if let Ok(id) = EdgeId::from_str(text) {
        assert_eq!(EdgeId::from_str(&id.to_string()), Ok(id));
    }
});
