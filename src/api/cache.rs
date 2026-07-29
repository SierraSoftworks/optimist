//! Remembering answers that cost real time to produce.
//!
//! # Why answers can be remembered at all
//!
//! Solving is arithmetic over thousands of draws against a design that is not
//! moving. The answer depends on the design as it stood, on the controls the
//! caller asked for, and on nothing else: the solver restarts its random stream
//! for every evaluation, so the same question asked twice has the same answer
//! both times. That makes the pair of them a key.
//!
//! The position in the change feed is part of that key rather than something
//! this cache watches for. An edit moves the design to a new position and
//! therefore to entries that do not exist yet, so a stale answer is unreachable
//! rather than invalidated. Nothing has to remember to clear anything, which is
//! the failure mode a cache keyed on mutable state always eventually has.
//!
//! # Why it is bounded
//!
//! Every edit to a design creates a new position, so an unbounded cache would
//! grow for as long as somebody is typing. The bound is on entries rather than
//! bytes because the thing being protected against is a long editing session
//! rather than one enormous answer, and because a caller cannot ask for an
//! answer larger than the draw budget already allows.
//!
//! Eviction is by least recent use, which suits the way the workbench asks:
//! somebody flicking between variants of the design in front of them returns to
//! the same handful of answers, while the positions they have edited past are
//! never asked for again.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// Answers held per design.
///
/// Sized for a working session rather than a history: enough to hold every
/// variant of a design at the position being looked at, plus the positions
/// somebody has stepped back through, and not so many that an afternoon of
/// editing retains every intermediate answer.
const CAPACITY: usize = 96;

/// A bounded store of computed answers, shared by everyone asking for them.
///
/// Keys are opaque strings built by the caller, which keeps the description of
/// what makes two requests the same next to the request type that knows.
pub(super) struct Answers<V> {
    capacity: usize,
    state: Mutex<State<V>>,
}

struct State<V> {
    entries: HashMap<String, Entry<V>>,
    /// Ticks once per read or write, giving every entry a distinct recency.
    clock: u64,
}

struct Entry<V> {
    value: Arc<V>,
    used: u64,
}

impl<V> Answers<V> {
    /// Creates a cache holding the default number of answers per design.
    pub(super) fn new() -> Self {
        Self::with_capacity(CAPACITY)
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            state: Mutex::new(State {
                entries: HashMap::new(),
                clock: 0,
            }),
        }
    }

    /// Returns a remembered answer, marking it as the most recently wanted.
    pub(super) fn get(&self, key: &str) -> Option<Arc<V>> {
        let mut state = self.lock();
        state.clock += 1;
        let now = state.clock;
        let entry = state.entries.get_mut(key)?;
        entry.used = now;
        Some(Arc::clone(&entry.value))
    }

    /// Remembers an answer, discarding the least recently wanted if full.
    ///
    /// Two callers racing on the same key both compute and both store. Storing
    /// twice is cheaper than holding a lock across a solve, and the answers are
    /// identical, so the loser of the race has wasted work rather than produced
    /// a disagreement.
    pub(super) fn insert(&self, key: String, value: Arc<V>) {
        let mut state = self.lock();
        state.clock += 1;
        let now = state.clock;
        if !state.entries.contains_key(&key) && state.entries.len() >= self.capacity {
            // A linear scan for the oldest entry, rather than the usual
            // intrusive list. At this capacity the scan is faster than
            // maintaining the list would be, and it happens once per eviction
            // rather than once per read.
            if let Some(oldest) = state
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(key, _)| key.clone())
            {
                state.entries.remove(&oldest);
            }
        }
        state.entries.insert(key, Entry { value, used: now });
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State<V>> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Answers for every design a server has been asked about.
///
/// Held per design so that one busy design cannot evict another's answers, and
/// so that the key each design stores under does not have to name it.
pub(super) struct Cache<V> {
    designs: Mutex<HashMap<String, Arc<Answers<V>>>>,
}

impl<V> Cache<V> {
    /// Creates an empty cache.
    pub(super) fn new() -> Self {
        Self {
            designs: Mutex::new(HashMap::new()),
        }
    }

    /// Returns the answers held for one design, creating the store if needed.
    pub(super) fn design(&self, design: &str) -> Arc<Answers<V>> {
        let mut designs = self
            .designs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Arc::clone(
            designs
                .entry(design.to_owned())
                .or_insert_with(|| Arc::new(Answers::new())),
        )
    }
    /// Drops everything remembered for one design.
    ///
    /// Positions in the change feed restart at zero for a design that is created
    /// under an identifier that has been used before, so answers from the design
    /// that is gone would otherwise be reachable from its replacement.
    pub(super) fn forget(&self, design: &str) {
        self.designs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(design);
    }
}

impl<V> Default for Cache<V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_is_returned_to_the_next_caller_that_asks() {
        let answers = Answers::new();
        answers.insert("k".to_owned(), Arc::new(7));
        assert_eq!(answers.get("k").as_deref(), Some(&7));
        assert!(answers.get("other").is_none());
    }

    /// The oldest entry goes, and reading one is what keeps it young.
    #[test]
    fn the_least_recently_wanted_answer_is_the_one_discarded() {
        let answers = Answers::<u32>::with_capacity(2);
        answers.insert("a".to_owned(), Arc::new(1));
        answers.insert("b".to_owned(), Arc::new(2));
        assert_eq!(answers.get("a").as_deref(), Some(&1));

        answers.insert("c".to_owned(), Arc::new(3));
        assert!(
            answers.get("b").is_none(),
            "the entry nobody asked for again must be the one evicted"
        );
        assert_eq!(answers.get("a").as_deref(), Some(&1));
        assert_eq!(answers.get("c").as_deref(), Some(&3));
    }

    /// Replacing a key must not count against the bound as a new entry would.
    #[test]
    fn rewriting_a_key_does_not_evict_anything() {
        let answers = Answers::<u32>::with_capacity(2);
        answers.insert("a".to_owned(), Arc::new(1));
        answers.insert("b".to_owned(), Arc::new(2));
        answers.insert("b".to_owned(), Arc::new(3));
        assert_eq!(answers.get("a").as_deref(), Some(&1));
        assert_eq!(answers.get("b").as_deref(), Some(&3));
    }

    /// One design's answers must not be reachable through another's name.
    #[test]
    fn designs_hold_their_own_answers() {
        let cache = Cache::<u32>::new();
        cache.design("left").insert("k".to_owned(), Arc::new(1));
        assert!(cache.design("right").get("k").is_none());
        assert_eq!(cache.design("left").get("k").as_deref(), Some(&1));
    }
}
