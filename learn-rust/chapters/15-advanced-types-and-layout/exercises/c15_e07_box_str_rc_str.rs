// c15_e07_box_str_rc_str.rs
//
// CONCEPT: `Box<str>` and `Arc<str>`: owned, immutable, cheaply shareable text; interning
// GUIDE:   chapters/15-advanced-types-and-layout/guide.md  (section "Box<str>, Arc<str> and interning")
// DATABROKER: databroker/src/broker.rs — `ChangeSubscription::notify` does
//             `update.path = Some(entry.metadata.path.clone())` for every changed
//             entry of every subscriber: one heap allocation + copy per notification.
//             With `Arc<str>` paths that clone would be a reference-count increment.
// EXPECT:  tests fail: `todo!()` panics
//
// A `String` is (pointer, length, capacity): growable. Most signal paths are
// never modified after loading, so the capacity word is wasted, and every
// `.clone()` copies all the bytes.
//   Box<str>  (pointer, length): owned, immutable, one word smaller.
//   Arc<str>  (pointer, length): shared. `.clone()` bumps a counter; no copy.
// Interning means storing each distinct string once and handing out shared
// handles (or small integer ids) to it.
//
// YOUR TASK
//   Implement `PathInterner` (the `todo!()`s) and `freeze`.
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::HashMap;
use std::sync::Arc;

/// Turns a growable `String` into an immutable `Box<str>`, dropping spare capacity.
fn freeze(s: String) -> Box<str> {
    todo!()
}

/// Stores each distinct path once. `intern` returns a shared handle plus a dense id.
#[derive(Default)]
struct PathInterner {
    // because `Arc<str>: Borrow<str>`. No allocation just to ask.
    ids: HashMap<Arc<str>, u32>,
    paths: Vec<Arc<str>>,
}

impl PathInterner {
    /// Returns the id and shared handle for `path`, storing it on first sight.
    fn intern(&mut self, path: &str) -> (u32, Arc<str>) {
        todo!()
    }

    /// Looks up the path for an id.
    fn resolve(&self, id: u32) -> Option<&str> {
        todo!()
    }

    fn len(&self) -> usize {
        todo!()
    }
}

/// A notification that carries its path as a shared handle.
#[derive(Clone, Debug)]
struct Notification {
    path: Arc<str>,
    value: i64,
}

/// Builds one notification per subscriber for the same changed path.
fn fan_out(path: &Arc<str>, value: i64, subscribers: usize) -> Vec<Notification> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn box_str_is_smaller_than_string() {
        assert_eq!(size_of::<Box<str>>(), 2 * size_of::<usize>());
        assert_eq!(size_of::<String>(), 3 * size_of::<usize>());
        let mut s = String::with_capacity(100);
        s.push_str("Vehicle.Speed");
        let frozen = freeze(s);
        assert_eq!(&*frozen, "Vehicle.Speed");
        assert_eq!(frozen.len(), 13);
    }

    #[test]
    fn interning_dedups_and_shares() {
        let mut interner = PathInterner::default();
        let (a_id, a) = interner.intern("Vehicle.Speed");
        let (b_id, _) = interner.intern("Vehicle.Width");
        let (c_id, c) = interner.intern("Vehicle.Speed");
        assert_eq!(a_id, c_id);
        assert_ne!(a_id, b_id);
        assert!(
            Arc::ptr_eq(&a, &c),
            "the same allocation must be handed out"
        );
        assert_eq!(interner.len(), 2);
        assert_eq!(interner.resolve(b_id), Some("Vehicle.Width"));
        assert_eq!(interner.resolve(99), None);
    }

    #[test]
    fn fan_out_shares_one_allocation() {
        let mut interner = PathInterner::default();
        let (_, path) = interner.intern("Vehicle.Cabin.Temperature");
        let before = Arc::strong_count(&path);
        let notes = fan_out(&path, 21, 50);
        assert_eq!(notes.len(), 50);
        assert!(notes
            .iter()
            .all(|n| Arc::ptr_eq(&n.path, &path) && n.value == 21));
        assert_eq!(Arc::strong_count(&path), before + 50);
        drop(notes);
        assert_eq!(Arc::strong_count(&path), before);
    }
}
