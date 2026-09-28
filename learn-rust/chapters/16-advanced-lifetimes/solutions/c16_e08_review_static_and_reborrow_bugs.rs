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
        // BUG 1: this always pushed a new entry, so `get`'s `find` (which
        // stops at the FIRST match) kept returning the stale original
        // value, and `entry_count` kept growing. Overwrite in place when
        // the key is already present.
        if let Some(existing) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            existing.1 = value;
        } else {
            self.entries.push((key, value));
        }
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
        // BUG 2: cloning the `Rc` gives every closure a handle to the SAME
        // cell, so calling any of them later reports whatever the counter
        // ended up at, not the value at the time this closure was made.
        // `'static` only requires the closure to *own* what it captures —
        // it says nothing about that data being shared or exclusive. Copy
        // the current value out instead, so each closure owns its own,
        // independent `i32`.
        let value = *counter.borrow();
        closures.push(Box::new(move || value));
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
// `v` stays `&Vec<i32>` (not `&[i32]`) to match the exercise's signature
// exactly; the fix below doesn't need any Vec-specific method.
#[allow(clippy::ptr_arg)]
fn quadruple(v: &mut Vec<i32>) {
    // BUG 3: `v.clone()` doubled a copy and left the original `v`
    // untouched. Passing `v` itself to `double_in_place` twice works
    // fine — Rust implicitly reborrows a `&mut` reference at each call
    // site, so `v` isn't moved out from under you.
    double_in_place(v);
    double_in_place(v);
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
