// c16_e08_review_static_and_reborrow_bugs.rs
//
// CONCEPT: code review. Three independent pieces of code, each compiling
//          cleanly, each with one planted bug typical of advanced-lifetime
//          code: a lifetime-carrying cache that duplicates instead of
//          updating, `'static` closures that share one mutable cell
//          instead of each owning an independent snapshot, and a helper
//          that clones instead of taking an implicit reborrow.
// GUIDE:   chapters/16-advanced-lifetimes/guide.md  (section "'static: a bound, not a lifetime of the value")
// DATABROKER: `SignalProvider: Send + Sync + 'static` (chapter 13) works
//          precisely because providers own their data instead of sharing
//          one mutable cell the way bug 2 below does.
// EXPECT:  compiles; 3 of the tests below fail.
//
// THIS FILE HAS 3 BUGS. All three compile cleanly.
//
// YOUR TASK
//   Find and fix all 3 bugs. Do not change the tests or the signatures.
//   For each bug, add a one-line comment directly above your fix:
//   `// BUG 1: <what was wrong>`, `// BUG 2: ...`, `// BUG 3: ...`.

// I AM NOT DONE

use std::cell::RefCell;
use std::rc::Rc;

// --- Bug 1: update duplicates instead of overwriting -----------------------

struct Cache<'a> {
    entries: Vec<(&'a str, i32)>,
}

impl<'a> Cache<'a> {
    fn new() -> Self {
        Cache {
            entries: Vec::new(),
        }
    }

    /// Sets `key`'s value, overwriting any existing entry for that key.
    fn update(&mut self, key: &'a str, value: i32) {
        self.entries.push((key, value));
    }

    fn get(&self, key: &str) -> Option<i32> {
        self.entries
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
    }

    fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

// --- Bug 2: 'static closures sharing one cell instead of a snapshot each --

/// Builds one `'static` closure per counter tick, `1..=n`. Each closure
/// must report the counter value it saw AT CREATION TIME, forever after —
/// not whatever the counter's value happens to be when it's later called.
fn make_snapshots(n: usize) -> Vec<Box<dyn Fn() -> i32>> {
    let counter = Rc::new(RefCell::new(0));
    let mut closures: Vec<Box<dyn Fn() -> i32>> = Vec::new();
    for _ in 0..n {
        *counter.borrow_mut() += 1;
        let shared = Rc::clone(&counter);
        closures.push(Box::new(move || *shared.borrow()));
    }
    closures
}

// --- Bug 3: cloning instead of reborrowing ---------------------------------

fn double_in_place(v: &mut [i32]) {
    for x in v.iter_mut() {
        *x *= 2;
    }
}

/// Doubles every element of `v` twice in place (net effect: `*4`).
fn quadruple(v: &mut Vec<i32>) {
    let mut copy = v.clone();
    double_in_place(&mut copy);
    double_in_place(&mut copy);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_update_overwrites_rather_than_duplicating() {
        let mut cache = Cache::new();
        cache.update("Vehicle.Speed", 10);
        cache.update("Vehicle.Speed", 20);
        assert_eq!(cache.get("Vehicle.Speed"), Some(20));
        assert_eq!(cache.entry_count(), 1);
    }

    #[test]
    fn make_snapshots_freezes_the_value_at_creation_time() {
        let snapshots = make_snapshots(3);
        let values: Vec<i32> = snapshots.iter().map(|f| f()).collect();
        assert_eq!(values, vec![1, 2, 3]);
    }

    #[test]
    fn quadruple_mutates_the_original_vec() {
        let mut v = vec![1, 2, 3];
        quadruple(&mut v);
        assert_eq!(v, vec![4, 8, 12]);
    }
}
