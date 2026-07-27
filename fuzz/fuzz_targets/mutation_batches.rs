#![no_main]

//! Fuzzes decoding of edit batches, which is the one place where bytes arrive
//! straight from the network. Every workbench edit is posted as JSON mutations
//! and applied to a live in-memory design, so a panic here is reachable by any
//! client that can open a socket.

mod json_limits;

use libfuzzer_sys::fuzz_target;
use optimist::session::Mutation;

fuzz_target!(|data: &[u8]| {
    // Reject deeply nested or oversized JSON up front so libfuzzer is not
    // rewarded for growing inputs that serde would spend all its time walking.
    if !json_limits::within_limits(data) {
        return;
    }

    let _ = serde_json::from_slice::<Vec<Mutation>>(data);
});
