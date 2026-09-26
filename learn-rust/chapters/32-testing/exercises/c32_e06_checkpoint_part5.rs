// c32_e06_checkpoint_part5.rs
//
// CONCEPT: CHECKPOINT for Part V. You write the tests. A good suite passes
//          on a correct implementation and FAILS on each buggy one.
// GUIDE:   chapters/32-testing/guide.md  (section "Checkpoint")
// DATABROKER: databroker/src/filter/filter_manager.rs — `FilterManager`:
//             `add_new_update_filter`, `remove_filter_by_subscription_uuid`
//             and the lowest-interval-per-signal computation
// EXPECT:  tests fail: `todo!()` panics in `check_filter`
//
// Several subscriptions can ask for the same signal at different sample
// intervals. The provider must be told the LOWEST interval per signal (the
// fastest anyone needs), and that must be recomputed when a subscription
// goes away. That's the databroker's `FilterManager`. The contract:
//
//   * `set(sub, signal, ms)` sets subscription `sub`'s interval for `signal`,
//     REPLACING any interval that subscription had for that signal.
//   * `remove_subscription(sub)` forgets everything `sub` asked for.
//   * `lowest()` returns, for every signal that some subscription still
//     wants, the minimum interval across those subscriptions.
//
// Three implementations follow: `Reference` is correct; `BugA` and `BugB`
// each break the contract in one subtle way. Try NOT to read the buggy ones
// until your suite catches them. Think about the contract instead:
// which sequences of calls would expose a careless implementation?
//
// YOUR TASK
//   Implement `check_filter`, a reusable test suite. It builds fresh filters
//   with `F::default()`, runs scenarios against them, and returns
//   `Err(description)` on the first contract violation it finds.
//   Tip: a *model-based* test (compare against a tiny obviously-correct
//   model over many generated call sequences, see the guide) catches bugs
//   you didn't think of. Hand-written scenarios are fine too.
//   Do not edit the implementations or the tests.

// I AM NOT DONE

#![allow(unused_imports)]
use std::collections::{BTreeMap, HashMap};

pub type SubId = u32;
pub type SignalId = i32;

pub trait IntervalFilter: Default {
    fn set(&mut self, sub: SubId, signal: SignalId, interval_ms: u32);
    fn remove_subscription(&mut self, sub: SubId);
    fn lowest(&self) -> BTreeMap<SignalId, u32>;
}

// ------------------------------------------------------------------------
// Your test suite.
// ------------------------------------------------------------------------

/// Returns `Ok(())` if `F` honours the contract in every scenario tried.
pub fn check_filter<F: IntervalFilter>() -> Result<(), String> {
    // Add any helpers you like above or below this function.
    todo!()
}

// ------------------------------------------------------------------------
// Implementations under test. Do not edit.
// ------------------------------------------------------------------------

/// Correct: remember every (subscription, signal) interval, compute on demand.
#[derive(Default)]
pub struct Reference {
    wanted: HashMap<(SubId, SignalId), u32>,
}

impl IntervalFilter for Reference {
    fn set(&mut self, sub: SubId, signal: SignalId, interval_ms: u32) {
        self.wanted.insert((sub, signal), interval_ms);
    }

    fn remove_subscription(&mut self, sub: SubId) {
        self.wanted.retain(|(s, _), _| *s != sub);
    }

    fn lowest(&self) -> BTreeMap<SignalId, u32> {
        let mut lowest = BTreeMap::new();
        for (&(_, signal), &ms) in &self.wanted {
            let entry = lowest.entry(signal).or_insert(ms);
            *entry = (*entry).min(ms);
        }
        lowest
    }
}

// Spoilers below this line.
// ------------------------------------------------------------------------

/// Caches the answer and recomputes it on `set`, but on removal it only
/// drops signals nobody wants any more: a removed subscription's faster
/// interval lingers in the cache.
#[derive(Default)]
pub struct BugA {
    wanted: HashMap<(SubId, SignalId), u32>,
    cached: BTreeMap<SignalId, u32>,
}

impl IntervalFilter for BugA {
    fn set(&mut self, sub: SubId, signal: SignalId, interval_ms: u32) {
        self.wanted.insert((sub, signal), interval_ms);
        let lowest = self
            .wanted
            .iter()
            .filter(|((_, s), _)| *s == signal)
            .map(|(_, ms)| *ms)
            .min()
            .expect("just inserted one");
        self.cached.insert(signal, lowest);
    }

    fn remove_subscription(&mut self, sub: SubId) {
        self.wanted.retain(|(s, _), _| *s != sub);
        let still_wanted: Vec<SignalId> = self.wanted.keys().map(|(_, signal)| *signal).collect();
        self.cached
            .retain(|signal, _| still_wanted.contains(signal));
    }

    fn lowest(&self) -> BTreeMap<SignalId, u32> {
        self.cached.clone()
    }
}

/// Treats `set` as "at least this fast": a subscription can lower its own
/// interval but never raise it again.
#[derive(Default)]
pub struct BugB {
    wanted: HashMap<(SubId, SignalId), u32>,
}

impl IntervalFilter for BugB {
    fn set(&mut self, sub: SubId, signal: SignalId, interval_ms: u32) {
        let entry = self.wanted.entry((sub, signal)).or_insert(interval_ms);
        *entry = (*entry).min(interval_ms);
    }

    fn remove_subscription(&mut self, sub: SubId) {
        self.wanted.retain(|(s, _), _| *s != sub);
    }

    fn lowest(&self) -> BTreeMap<SignalId, u32> {
        let mut lowest = BTreeMap::new();
        for (&(_, signal), &ms) in &self.wanted {
            let entry = lowest.entry(signal).or_insert(ms);
            *entry = (*entry).min(ms);
        }
        lowest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suite_accepts_the_correct_implementation() {
        assert_eq!(check_filter::<Reference>(), Ok(()));
    }

    #[test]
    fn suite_catches_bug_a() {
        let result = check_filter::<BugA>();
        assert!(result.is_err(), "BugA slipped through your suite");
        println!("BugA caught: {}", result.unwrap_err());
    }

    #[test]
    fn suite_catches_bug_b() {
        let result = check_filter::<BugB>();
        assert!(result.is_err(), "BugB slipped through your suite");
        println!("BugB caught: {}", result.unwrap_err());
    }
}
