//! Damping a pass's result toward the value it is converging on.

use std::collections::BTreeMap;

use rand_chacha::ChaCha20Rng;

use crate::{
    squiggle::Value,
    system::{
        compile::{PreparedComponent, PreparedPort},
        values::{blend, distance, draws, from_draws},
    },
};

use super::{config::EvaluationConfig, state::ComponentState};

pub(super) fn converge(
    component: &PreparedComponent,
    settled: &ComponentState,
    computed: &ComponentState,
    weight: f64,
    config: EvaluationConfig,
    rng: &mut ChaCha20Rng,
) -> (ComponentState, f64) {
    let mut blended = ComponentState::default();
    let mut moved: f64 = 0.0;
    for (name, next) in &computed.channels {
        let Some(previous) = settled.channels.get(name) else {
            // Nothing to blend against on the first pass, so the computed value
            // stands and the step cannot yet be treated as settled.
            blended.channels.insert(name.clone(), next.clone());
            moved = f64::INFINITY;
            continue;
        };
        let count = config.sample_count;
        let (Some(previous), Some(next)) = (draws(previous, count, rng), draws(next, count, rng))
        else {
            blended.channels.insert(name.clone(), next.clone());
            continue;
        };
        moved = moved.max(distance(&previous, &next));
        let value = from_draws(blend(&previous, &next, weight)).unwrap_or(Value::Number(0.0));
        blended.channels.insert(name.clone(), value);
    }
    // A port publishes quantities derived from the channels, so it follows
    // whichever blended channel it names. Values that do not correspond to a
    // channel are constant and are carried through as computed.
    blended.requests = republish(&component.outbound, &blended.channels, &computed.requests);
    blended.responses = republish(&component.inbound, &blended.channels, &computed.responses);
    blended.arriving = computed.arriving.clone();
    blended.returning = computed.returning.clone();
    (blended, moved)
}

/// Re-derives each port's published signals from the blended channels.
///
/// A publication that names a channel outright follows that channel's blended
/// value, so the figure travelling the wire is damped exactly as the quantity it
/// reports is. Anything computed some other way is carried through as evaluated
/// and settles on the next pass.
fn republish(
    ports: &BTreeMap<String, PreparedPort>,
    channels: &BTreeMap<String, Value>,
    computed: &BTreeMap<String, BTreeMap<String, Value>>,
) -> BTreeMap<String, BTreeMap<String, Value>> {
    ports
        .iter()
        .map(|(name, port)| {
            let published = port
                .publishes
                .iter()
                .filter_map(|(signal, source, _)| {
                    let value = channels
                        .get(source)
                        .or_else(|| computed.get(name).and_then(|signals| signals.get(signal)))?;
                    Some((signal.clone(), value.clone()))
                })
                .collect::<BTreeMap<_, _>>();
            (name.clone(), published)
        })
        .collect()
}
