#![no_main]

mod json_limits;

use libfuzzer_sys::fuzz_target;
use optimist::domain::{EstimateAddress, Formula, FormulaSet, ProjectId, Unit};

fuzz_target!(|data: &[u8]| {
    if !json_limits::within_limits(data) {
        return;
    }
    if let Ok(unit) = serde_json::from_slice::<Unit>(data) {
        let encoded = serde_json::to_vec(&unit).expect("validated unit serializes");
        let decoded: Unit = serde_json::from_slice(&encoded).expect("unit round trip decodes");
        assert_eq!(unit, decoded);
    }
    if let Ok(address) = serde_json::from_slice::<EstimateAddress>(data) {
        let text = address.to_string();
        assert_eq!(
            text.parse::<EstimateAddress>()
                .expect("address text decodes"),
            address
        );
    }
    if let Ok(formula) = serde_json::from_slice::<Formula>(data) {
        let encoded = serde_json::to_vec(&formula).expect("formula serializes");
        let decoded: Formula =
            serde_json::from_slice(&encoded).expect("formula round trip decodes");
        assert_eq!(formula, decoded);
        let project = ProjectId::new("fuzz").expect("valid project");
        let _ = FormulaSet::default().validate(&project, &formula);
    }
});
