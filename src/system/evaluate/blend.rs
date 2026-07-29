//! Damping a pass's result toward the value it is converging on.

use std::collections::BTreeMap;

use rand_chacha::ChaCha20Rng;

use crate::{
    squiggle::Value,
    system::{
        compile::{PreparedComponent, PreparedPort},
        values::{Stride, Varying, converge as settle},
    },
};

use super::{config::EvaluationConfig, state::ComponentState};

pub(super) fn converge(
    component: &PreparedComponent,
    settled: &ComponentState,
    computed: &ComponentState,
    stride: &Stride,
    moved: &mut [f64],
    config: EvaluationConfig,
    rng: &mut ChaCha20Rng,
) -> (ComponentState, Moved) {
    let mut blended = ComponentState::default();
    let mut furthest = Moved::default();
    for (name, next) in &computed.channels {
        let Some(settled) = settled.channels.get(name) else {
            // Nothing to blend against on the first pass, so the computed value
            // stands and the step cannot yet be treated as settled.
            blended.channels.insert(name.clone(), next.clone());
            furthest.record(f64::INFINITY, name);
            moved.fill(f64::INFINITY);
            continue;
        };
        let count = config.ensemble().len();
        let (Some(settled), Some(computed)) = (
            Varying::of(settled, config.ensemble(), rng),
            Varying::of(next, config.ensemble(), rng),
        ) else {
            blended.channels.insert(name.clone(), next.clone());
            continue;
        };
        let (value, gap) = settle(&settled, &computed, stride, count, moved);
        furthest.record(gap, name);
        blended.channels.insert(name.clone(), value);
    }
    // A port publishes quantities derived from the channels, so it follows
    // whichever blended channel it names. Values that do not correspond to a
    // channel are constant and are carried through as computed.
    blended.requests = republish(&component.outbound, &blended.channels, &computed.requests);
    blended.responses = republish(&component.inbound, &blended.channels, &computed.responses);
    blended.arriving = computed.arriving.clone();
    blended.returning = computed.returning.clone();
    (blended, furthest)
}

/// How far a component moved, and which of its channels moved furthest.
///
/// The name is what makes an unsettled solve actionable. "Nothing settled" sends
/// an author looking through the whole design; "utilisation is still moving by a
/// tenth every pass" sends them to the loop that is not closing.
#[derive(Clone, Debug, Default)]
pub(super) struct Moved {
    pub(super) distance: f64,
    pub(super) channel: Option<String>,
}

impl Moved {
    fn record(&mut self, distance: f64, channel: &str) {
        if distance > self.distance || self.channel.is_none() {
            self.distance = distance;
            self.channel = Some(channel.to_owned());
        }
    }
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
