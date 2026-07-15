#![no_main]

mod json_limits;

use libfuzzer_sys::fuzz_target;
use optimist::{command::CommandRequest, project::ProjectCatalog};

const MAX_COMMANDS: usize = 16;

fuzz_target!(|data: &[u8]| {
    if !json_limits::within_limits(data) {
        return;
    }
    let Ok(requests) = serde_json::from_slice::<Vec<CommandRequest>>(data) else {
        return;
    };
    if requests.len() > MAX_COMMANDS {
        return;
    }

    let mut left = ProjectCatalog::new();
    let mut right = ProjectCatalog::new();
    let left_project = left
        .create("fuzz-project".to_owned())
        .expect("create project");
    let right_project = right
        .create("fuzz-project".to_owned())
        .expect("create project");

    for request in requests {
        let encoded = serde_json::to_vec(&request).expect("decoded command must serialize");
        let decoded = serde_json::from_slice(&encoded).expect("serialized command must decode");
        assert_eq!(request, decoded);

        let left_result = left.execute(&left_project.id, request.clone());
        let right_result = right.execute(&right_project.id, request.clone());
        assert_eq!(left_result, right_result);
        assert_eq!(left_result, left.execute(&left_project.id, request));
    }

    assert_eq!(left.get(&left_project.id), right.get(&right_project.id));
    assert_eq!(
        left.list_nodes(&left_project.id),
        right.list_nodes(&right_project.id)
    );
    assert_eq!(
        left.list_edges(&left_project.id),
        right.list_edges(&right_project.id)
    );
});
