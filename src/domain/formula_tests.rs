use proptest::prelude::*;

use super::{
    Distribution, EntityId, EstimateAddress, EstimateComponentId, EstimateId, EstimateOwner,
    Formula, FormulaError, FormulaSet, ProjectId, Unit,
};

fn project() -> ProjectId {
    ProjectId::new("forecast").unwrap()
}

fn address(id: u64) -> EstimateAddress {
    EstimateAddress::new(
        project(),
        EstimateOwner::Node(EntityId::new(id)),
        EstimateId::new(0),
    )
}

fn literal(unit: Unit) -> Formula {
    Formula::Literal {
        distribution: Distribution::point(1.0).unwrap(),
        unit,
    }
}

#[test]
fn derives_units_and_deterministic_shared_dependencies() {
    let distance = address(0);
    let time = address(1);
    let formulas = FormulaSet::new([
        (distance.clone(), literal(Unit::base("m").unwrap())),
        (time.clone(), literal(Unit::base("s").unwrap())),
    ])
    .unwrap();
    let speed = Formula::Ratio {
        numerator: Box::new(Formula::Reference {
            address: distance.clone(),
        }),
        denominator: Box::new(Formula::Product {
            factors: vec![
                Formula::Reference {
                    address: time.clone(),
                },
                Formula::Reference {
                    address: time.clone(),
                },
            ],
        }),
    };
    let compiled = formulas.validate(&project(), &speed).unwrap();
    assert_eq!(compiled.unit.exponent("m"), 1);
    assert_eq!(compiled.unit.exponent("s"), -2);
    assert_eq!(compiled.dependencies, vec![distance, time]);
}

#[test]
fn rejects_unit_mismatch_missing_cross_project_and_cycles() {
    let left = address(0);
    let right = address(1);
    let mismatch = FormulaSet::new([
        (left.clone(), literal(Unit::base("m").unwrap())),
        (right.clone(), literal(Unit::base("s").unwrap())),
    ])
    .unwrap();
    assert!(matches!(
        mismatch.validate(
            &project(),
            &Formula::Sum {
                terms: vec![
                    Formula::Reference {
                        address: left.clone()
                    },
                    Formula::Reference {
                        address: right.clone()
                    }
                ]
            }
        ),
        Err(FormulaError::UnitMismatch { .. })
    ));
    assert!(matches!(
        FormulaSet::default().validate(
            &project(),
            &Formula::Reference {
                address: left.clone()
            }
        ),
        Err(FormulaError::MissingReference(_))
    ));
    let foreign = EstimateAddress::new(
        ProjectId::new("other").unwrap(),
        EstimateOwner::Node(EntityId::new(0)),
        EstimateId::new(0),
    );
    assert!(matches!(
        mismatch.validate(&project(), &Formula::Reference { address: foreign }),
        Err(FormulaError::CrossProjectReference { .. })
    ));
    let cyclic = FormulaSet::new([
        (
            left.clone(),
            Formula::Reference {
                address: right.clone(),
            },
        ),
        (right.clone(), Formula::Reference { address: left }),
    ])
    .unwrap();
    assert!(matches!(
        cyclic.validate(&project(), &Formula::Reference { address: right }),
        Err(FormulaError::ReferenceCycle(_))
    ));
}

#[test]
fn rejects_invalid_arity_and_bounds() {
    let formulas = FormulaSet::default();
    assert_eq!(
        formulas.validate(&project(), &Formula::Sum { terms: vec![] }),
        Err(FormulaError::TooFewOperands { operation: "sum" })
    );
    assert_eq!(
        formulas.validate(
            &project(),
            &Formula::Bounded {
                input: Box::new(literal(Unit::dimensionless())),
                lower: 2.0,
                upper: 1.0,
            }
        ),
        Err(FormulaError::InvalidBounds)
    );
}

#[test]
fn address_component_text_and_json_round_trip() {
    let value = address(42).with_component(EstimateComponentId::new("labor.hours").unwrap());
    assert_eq!(value.to_string().parse::<EstimateAddress>().unwrap(), value);
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(
        serde_json::from_str::<EstimateAddress>(&json).unwrap(),
        value
    );
}

proptest! {
    #[test]
    fn formula_serde_round_trip(value in -1_000_000_i32..1_000_000) {
        let formula = Formula::Power {
            base: Box::new(Formula::Literal {
                distribution: Distribution::point(f64::from(value)).unwrap(),
                unit: Unit::base("widget").unwrap(),
            }),
            exponent: 2,
        };
        let json = serde_json::to_string(&formula).unwrap();
        prop_assert_eq!(serde_json::from_str::<Formula>(&json).unwrap(), formula);
    }
}
