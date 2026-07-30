//! Counting the work a solve does, so a speed-up can be attributed.
//!
//! Wall-clock time says a change helped. It does not say whether it helped
//! because the solver took fewer passes or because each pass got cheaper, and
//! those two call for opposite follow-up work. These counters separate them.
//!
//! Everything here compiles away unless the `profiling` feature is on, so the
//! call sites can sit directly in the hot path. Counters are process-wide and
//! relaxed, because the question they answer is the ratio between two kinds of
//! work over a whole solve rather than a precisely ordered sequence of events.

/// Adds to a counter, or expands to nothing when profiling is off.
#[cfg(feature = "profiling")]
macro_rules! count {
    ($counter:ident) => {
        $crate::profile::bump($crate::profile::Counter::$counter, 1);
    };
    ($counter:ident, $amount:expr) => {
        $crate::profile::bump($crate::profile::Counter::$counter, $amount as u64);
    };
}

/// Adds to a counter, or expands to nothing when profiling is off.
#[cfg(not(feature = "profiling"))]
macro_rules! count {
    ($($ignored:tt)*) => {};
}

/// Times an expression, or evaluates it untouched when profiling is off.
///
/// Phases nest, so a total is not a partition of the solve. Read each against
/// the whole rather than adding them together.
#[cfg(feature = "profiling")]
macro_rules! time {
    ($phase:ident, $body:expr) => {{
        let started = std::time::Instant::now();
        let outcome = $body;
        $crate::profile::spend($crate::profile::Phase::$phase, started.elapsed());
        outcome
    }};
}

/// Times an expression, or evaluates it untouched when profiling is off.
#[cfg(not(feature = "profiling"))]
macro_rules! time {
    ($phase:ident, $body:expr) => {
        $body
    };
}

pub(crate) use {count, time};

#[cfg(feature = "profiling")]
pub use enabled::{Counter, Counts, Phase, Spans, bump, reset, snapshot, spans, spend};

#[cfg(feature = "profiling")]
mod enabled {
    use std::sync::atomic::{AtomicU64, Ordering};

    /// One kind of work a solve performs.
    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub enum Counter {
        /// Relaxation passes, summed over every step of the horizon.
        Passes,
        /// Component evaluations: passes multiplied by the component count.
        Components,
        /// Squiggle programs evaluated, one per channel and per published signal.
        Programs,
        /// Name lookups resolved against the scope chain.
        Lookups,
        /// Dictionary entries copied by those lookups.
        ///
        /// A lookup hands back a clone, so naming `in`, `out` or `prev` copies
        /// the whole nested map before one field is read out of it. Counting the
        /// entries rather than the lookups is what tells an expensive name from
        /// a cheap one.
        LookupEntries,
        /// Calls into the elementwise driver.
        Elementwise,
        /// Individual draws produced, across sampling, blending and aggregation.
        Draws,
    }

    impl Counter {
        /// Every counter, in declaration order.
        pub const ALL: [Self; 7] = [
            Self::Passes,
            Self::Components,
            Self::Programs,
            Self::Lookups,
            Self::LookupEntries,
            Self::Elementwise,
            Self::Draws,
        ];
    }

    /// The value of every counter at one moment.
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct Counts([u64; Counter::ALL.len()]);

    impl Counts {
        /// Reads one counter.
        pub fn get(&self, counter: Counter) -> u64 {
            self.0[counter as usize]
        }

        /// Pairs each counter with its value, for reporting.
        pub fn entries(&self) -> impl Iterator<Item = (Counter, u64)> {
            let counts = *self;
            Counter::ALL
                .into_iter()
                .map(move |counter| (counter, counts.get(counter)))
        }
    }

    static COUNTS: [AtomicU64; Counter::ALL.len()] =
        [const { AtomicU64::new(0) }; Counter::ALL.len()];

    /// Adds to a counter.
    pub fn bump(counter: Counter, amount: u64) {
        COUNTS[counter as usize].fetch_add(amount, Ordering::Relaxed);
    }

    /// Reads every counter without disturbing it.
    pub fn snapshot() -> Counts {
        let mut counts = Counts::default();
        for counter in Counter::ALL {
            counts.0[counter as usize] = COUNTS[counter as usize].load(Ordering::Relaxed);
        }
        counts
    }

    /// Returns every counter to zero.
    pub fn reset() {
        for slot in &COUNTS {
            slot.store(0, Ordering::Relaxed);
        }
        for slot in &NANOS {
            slot.store(0, Ordering::Relaxed);
        }
    }

    /// One stretch of a solve worth timing separately.
    ///
    /// These nest: `Channels` runs inside `Relax`, which runs inside the
    /// horizon that `Prepare` is charged against. The total of every phase is
    /// therefore larger than the solve, and each is meaningful only against the
    /// whole rather than against its siblings.
    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub enum Phase {
        /// Resolving the plan, which happens once per step of the horizon.
        Prepare,
        /// Everything one step's relaxation does.
        Relax,
        /// Gathering what arrives on a component's ports, queues included.
        Arrivals,
        /// Copying flow dictionaries around inside that gathering.
        Gather,
        /// Running the behaviours attached to a relationship.
        Behaviours,
        /// Solving or carrying the queue on a wire.
        Queue,
        /// Reducing several arrivals of one signal to the figure a component reads.
        Combine,
        /// Evaluating a component's channels and published signals.
        Channels,
        /// Damping a computed component toward the value it is converging on.
        Converge,
    }

    impl Phase {
        /// Every phase, in declaration order.
        pub const ALL: [Self; 9] = [
            Self::Prepare,
            Self::Relax,
            Self::Arrivals,
            Self::Gather,
            Self::Behaviours,
            Self::Queue,
            Self::Combine,
            Self::Channels,
            Self::Converge,
        ];
    }

    /// How long each phase took, summed over every thread.
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct Spans([u64; Phase::ALL.len()]);

    impl Spans {
        /// Reads one phase's total.
        pub fn get(&self, phase: Phase) -> std::time::Duration {
            std::time::Duration::from_nanos(self.0[phase as usize])
        }

        /// Pairs each phase with its total, for reporting.
        pub fn entries(&self) -> impl Iterator<Item = (Phase, std::time::Duration)> {
            let spans = *self;
            Phase::ALL
                .into_iter()
                .map(move |phase| (phase, spans.get(phase)))
        }
    }

    static NANOS: [AtomicU64; Phase::ALL.len()] = [const { AtomicU64::new(0) }; Phase::ALL.len()];

    /// Charges a duration against a phase.
    pub fn spend(phase: Phase, elapsed: std::time::Duration) {
        NANOS[phase as usize].fetch_add(elapsed.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Reads every phase's total without disturbing it.
    pub fn spans() -> Spans {
        let mut spans = Spans::default();
        for phase in Phase::ALL {
            spans.0[phase as usize] = NANOS[phase as usize].load(Ordering::Relaxed);
        }
        spans
    }
}
