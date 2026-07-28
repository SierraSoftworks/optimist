//! Evaluating and ranking constraints in a solved model.

use std::collections::BTreeMap;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

use crate::{
    squiggle::Value,
    system::{
        compile::{Timing, prepare, runtime},
        evaluate::{EvaluationConfig, EvaluationError, SolveMode, Step},
        expression::{INBOUND, OUTBOUND, PREVIOUS, STEADY, STEP, TIME},
        manifest::ComponentType,
        model::{ComponentId, SystemModel},
        mutator::Mutator,
        values::draws,
    },
};

use super::Bottleneck;

pub(in crate::system) fn rank(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    mutators: &BTreeMap<String, Mutator>,
    overrides: &BTreeMap<String, String>,
    step: &Step,
    config: EvaluationConfig,
) -> Result<Vec<Bottleneck>, EvaluationError> {
    let plan = prepare(
        model,
        catalogue,
        mutators,
        overrides,
        Timing {
            seed: config.seed,
            sample_count: config.sample_count,
            time: step.time,
            step: config.step,
        },
    )?;
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
    let mut ranked = Vec::new();
    for component in &plan.components {
        let Some(state) = step.components.get(&component.id) else {
            continue;
        };
        let mut scope = plan.globals.clone();
        scope.extend(component.properties.clone());
        scope.extend(state.channels.clone());
        scope.insert(TIME.to_owned(), Value::Number(step.time));
        scope.insert(STEP.to_owned(), Value::Number(config.step));
        scope.insert(
            STEADY.to_owned(),
            Value::Boolean(config.mode == SolveMode::Steady),
        );
        scope.insert(INBOUND.to_owned(), Value::Dictionary(BTreeMap::new()));
        scope.insert(OUTBOUND.to_owned(), Value::Dictionary(BTreeMap::new()));
        scope.insert(PREVIOUS.to_owned(), Value::Dictionary(BTreeMap::new()));

        let mut runtime = runtime(config.seed, config.sample_count)?;
        for (name, (demand, limit)) in &component.constraints {
            let location = || format!("constraint '{name}' of component '{}'", component.id);
            let bindings = || {
                scope
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.clone()))
            };
            let demand = runtime
                .evaluate_values(demand, bindings())
                .map_err(|error| EvaluationError::Evaluation {
                    location: location(),
                    message: error.message,
                })?;
            let limit = runtime
                .evaluate_values(limit, bindings())
                .map_err(|error| EvaluationError::Evaluation {
                    location: location(),
                    message: error.message,
                })?;
            let (Some(demand), Some(limit)) = (
                draws(&demand, config.sample_count, &mut rng),
                draws(&limit, config.sample_count, &mut rng),
            ) else {
                continue;
            };
            ranked.push(measure(
                component.id.clone(),
                None,
                name.clone(),
                component.component_type.constraints[name].summary.clone(),
                component.replicas,
                &demand,
                &limit,
            ));
        }
    }
    for (id, state) in &step.links {
        let (Some(transfer), Some(bandwidth)) = (
            draws(&state.transfer, config.sample_count, &mut rng),
            draws(&state.bandwidth, config.sample_count, &mut rng),
        ) else {
            continue;
        };
        // A link nobody gave a speed to is not a link that is full.
        if bandwidth.iter().all(|limit| limit.is_infinite()) {
            continue;
        }
        ranked.push(measure(
            id.from.clone(),
            Some(id.to_string()),
            "bandwidth".to_owned(),
            "Bytes crossing the relationship against how fast it carries them. \
             Saturating means the link is the bottleneck rather than either end \
             of it, which is the reading that sends somebody to the network \
             rather than to the service."
                .to_owned(),
            1.0,
            &transfer,
            &bandwidth,
        ));
    }
    ranked.sort_by(|left, right| {
        right
            .probability_of_binding
            .total_cmp(&left.probability_of_binding)
            .then(right.utilisation.total_cmp(&left.utilisation))
            .then(left.component.as_str().cmp(right.component.as_str()))
            .then(left.link.cmp(&right.link))
            .then(left.constraint.cmp(&right.constraint))
    });
    Ok(ranked)
}

#[allow(clippy::too_many_arguments)]
fn measure(
    component: ComponentId,
    link: Option<String>,
    constraint: String,
    summary: String,
    replicas: f64,
    demand: &[f64],
    limit: &[f64],
) -> Bottleneck {
    let count = demand.len().min(limit.len()).max(1);
    let mut ratios = Vec::with_capacity(count);
    let mut binding = 0_usize;
    let mut demand_total = 0.0;
    let mut limit_total = 0.0;
    for (demand, limit) in demand.iter().zip(limit).take(count) {
        demand_total += demand;
        limit_total += limit;
        if demand >= limit {
            binding += 1;
        }
        // A limit of zero admits no demand at all, so any demand against it is
        // fully saturating rather than undefined.
        ratios.push(if *limit == 0.0 {
            f64::from(u8::from(*demand > 0.0))
        } else {
            demand / limit
        });
    }
    ratios.sort_by(f64::total_cmp);
    let utilisation = ratios.iter().sum::<f64>() / count as f64;
    let index = ((count as f64 * 0.9).ceil() as usize).clamp(1, count) - 1;
    Bottleneck {
        component,
        link,
        constraint,
        summary,
        replicas,
        utilisation,
        utilisation_p90: ratios[index],
        probability_of_binding: binding as f64 / count as f64,
        headroom: (limit_total - demand_total) / count as f64,
    }
}
