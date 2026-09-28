// c12_e10_review_shared_state_bugs.rs
//
// CONCEPT: code review. The three pieces of code below all compile. Each
//          has one planted bug typical of `Rc`/`RefCell` code: a runtime
//          double-borrow panic, an `Rc` cycle that leaks memory instead of
//          using `Weak`, and a "shared" handle that's secretly an
//          independent copy.
// GUIDE:   chapters/12-smart-pointers/guide.md  (section "Rc and RefCell: shared, single-threaded")
// DATABROKER: databroker/src/broker.rs's `AuthorizedAccess` is careful to
//          never hold two overlapping borrows of the same lock at once —
//          the first bug below is the single-threaded, panicking version of
//          that same mistake.
// EXPECT:  compiles; some tests fail, and one panics with "already borrowed"
//          until you fix bug 1.
//
// THIS FILE HAS 3 BUGS. All three compile cleanly.
//
// YOUR TASK
//   Find and fix all 3 bugs. Do not change the tests. You MAY change field
//   types (bug 2 specifically needs it). For each bug, add a one-line
//   comment directly above your fix: `// BUG 1: <what was wrong>`,
//   `// BUG 2: ...`, `// BUG 3: ...`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

// --- Bug 1: double borrow -------------------------------------------------

struct Sensor {
    readings: RefCell<Vec<i32>>,
}

fn total(sensor: &Rc<Sensor>) -> i32 {
    sensor.readings.borrow().iter().sum()
}

/// Records `value`, then returns the new running total.
fn record_and_total(sensor: &Rc<Sensor>, value: i32) -> i32 {
    // BUG 1: the `borrow_mut()` guard was still alive (bound to `readings`)
    // when `total` tried to `borrow()` the same `RefCell`, which panics at
    // runtime ("already mutably borrowed"). Let the mutable borrow end
    // (it's a temporary now, dropped at the end of the statement) before
    // calling `total`.
    sensor.readings.borrow_mut().push(value);
    total(sensor)
}

// --- Bug 2: Rc cycle -------------------------------------------------------

struct Node {
    name: String,
    // BUG 2: a child pointing at its parent with a *strong* `Rc` — combined
    // with the parent's strong `Rc` to the child — is a reference cycle:
    // neither strong count can ever reach zero, so neither is ever dropped.
    // A parent link should be `Weak`; only children need to be owned.
    parent: RefCell<Option<Weak<Node>>>,
    children: RefCell<Vec<Rc<Node>>>,
}

fn attach_child(parent: &Rc<Node>, child: &Rc<Node>) {
    *child.parent.borrow_mut() = Some(Rc::downgrade(parent));
    parent.children.borrow_mut().push(Rc::clone(child));
}

// --- Bug 3: accidental copy instead of a shared handle --------------------

#[derive(Default)]
struct Cache {
    entries: HashMap<String, Rc<RefCell<i32>>>,
}

impl Cache {
    /// Returns a handle to the *same* cell already stored under `path`
    /// (creating one, initialised to 0, if `path` is new) — mutating either
    /// handle must be visible through the other.
    fn mirror(&mut self, path: &str) -> Rc<RefCell<i32>> {
        let cell = self
            .entries
            .entry(path.to_string())
            .or_insert_with(|| Rc::new(RefCell::new(0)));
        // BUG 3: reading the value out and wrapping it in a brand-new
        // `Rc<RefCell<_>>` makes an independent copy, not a shared handle.
        // `Rc::clone` bumps the refcount and returns a handle to the *same*
        // cell instead.
        Rc::clone(cell)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_total_does_not_panic() {
        let sensor = Rc::new(Sensor {
            readings: RefCell::new(Vec::new()),
        });
        assert_eq!(record_and_total(&sensor, 3), 3);
        assert_eq!(record_and_total(&sensor, 4), 7);
    }

    #[test]
    fn dropping_both_ends_frees_the_whole_node_pair() {
        let parent = Rc::new(Node {
            name: "parent".to_string(),
            parent: RefCell::new(None),
            children: RefCell::new(Vec::new()),
        });
        let child = Rc::new(Node {
            name: "child".to_string(),
            parent: RefCell::new(None),
            children: RefCell::new(Vec::new()),
        });
        attach_child(&parent, &child);

        let parent_weak = Rc::downgrade(&parent);
        drop(parent);
        drop(child);

        // If nothing but the (now-dropped) locals kept these alive, the
        // weak handle should no longer upgrade.
        assert!(parent_weak.upgrade().is_none());
    }

    #[test]
    fn mirror_shares_state_with_the_original() {
        let mut cache = Cache::default();
        let original = cache
            .entries
            .entry("x".to_string())
            .or_insert_with(|| Rc::new(RefCell::new(1)))
            .clone();
        let mirrored = cache.mirror("x");
        *original.borrow_mut() += 1;
        assert_eq!(*mirrored.borrow(), 2);
    }

    #[test]
    fn mirror_of_a_new_path_starts_at_zero() {
        let mut cache = Cache::default();
        let mirrored = cache.mirror("new");
        assert_eq!(*mirrored.borrow(), 0);
    }
}
