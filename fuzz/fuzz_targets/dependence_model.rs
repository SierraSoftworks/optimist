#![no_main]

mod json_limits;

use libfuzzer_sys::fuzz_target;
use optimist::domain::{ProjectDependenceModel, ProjectId};

const MAX_GROUPS: usize = 8;
const MAX_MEMBERS: usize = 16;

fuzz_target!(|data: &[u8]| {
    if !json_limits::within_limits(data) {
        return;
    }
    let Ok(model) = serde_json::from_slice::<ProjectDependenceModel>(data) else {
        return;
    };
    if model.residual_groups.len() > MAX_GROUPS
        || model.residual_groups.iter().any(|group| {
            group.members.len() > MAX_MEMBERS
                || group.correlation.matrix.len() > MAX_MEMBERS
                || group
                    .correlation
                    .matrix
                    .iter()
                    .any(|row| row.len() > MAX_MEMBERS)
        })
    {
        return;
    }
    let encoded = serde_json::to_vec(&model).expect("decoded dependence must serialize");
    let decoded: ProjectDependenceModel =
        serde_json::from_slice(&encoded).expect("serialized dependence must decode");
    assert_eq!(model, decoded);
    for project in [
        ProjectId::new("fuzz").unwrap(),
        ProjectId::new("other").unwrap(),
    ] {
        let first = model.validate_for_project(&project);
        let second = model.validate_for_project(&project);
        assert_eq!(first, second);
    }
});
