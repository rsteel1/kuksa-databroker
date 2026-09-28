// c03_e08_review_signal_registry.rs
//
// CONCEPT: code review — this file COMPILES and looks reasonable, but has
//          3 planted bugs typical of chapter 03 (a clone that hides a
//          missed mutation, an off-by-one index, a shadowed counter
//          field).
// GUIDE:   chapters/03-structs/guide.md  (section "Associated functions: new and Default")
// DATABROKER: databroker/src/broker.rs — `Database::next_id` assigns ids
//             to new entries, and `Entry::apply` mutates entries in
//             place; a clone that isn't written back (bug 1 here) would
//             silently drop every update.
// EXPECT:  tests fail: an update doesn't stick, ids stop increasing, and
//          `nth_most_recent` returns the wrong value.
//
// A tiny in-memory registry of signal entries, keyed by an id assigned at
// registration time.
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not edit the tests, and do not change any method's signature.

#[derive(Clone)]
struct Entry {
    id: i32,
    path: String,
    value: f64,
}

struct Registry {
    entries: Vec<Entry>,
    next_id: i32,
}

impl Registry {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 0,
        }
    }

    /// Registers a new entry and returns its freshly assigned id.
    fn register(&mut self, path: &str, value: f64) -> i32 {
        let id = self.next_id;
        // BUG 3: `let next_id = ...` shadowed a local instead of updating
        // the `next_id` field, so it stayed 0 forever and every entry got
        // id 0. Assign into `self.next_id`.
        self.next_id = id + 1;
        self.entries.push(Entry {
            id,
            path: path.to_string(),
            value,
        });
        id
    }

    /// Overwrites the value of the entry with `id`. Returns whether an
    /// entry with that id existed.
    fn update(&mut self, id: i32, value: f64) -> bool {
        // BUG 1: cloning the found entry and mutating the clone never
        // wrote the new value back into `self.entries`. Find a `&mut`
        // reference into the Vec and mutate through it instead.
        match self.entries.iter_mut().find(|e| e.id == id) {
            Some(entry) => {
                entry.value = value;
                true
            }
            None => false,
        }
    }

    /// The current value of the entry with `id`, if it exists.
    fn get(&self, id: i32) -> Option<f64> {
        self.entries.iter().find(|e| e.id == id).map(|e| e.value)
    }

    /// The value of the `n`th most recently registered entry
    /// (`n = 0` is the most recent one).
    fn nth_most_recent(&self, n: usize) -> Option<f64> {
        // BUG 2: `len() - n` is one past the right index (for `n = 0` it's
        // `len()`, one past the last element). The most recent entry is
        // at `len() - 1 - n`.
        self.entries
            .len()
            .checked_sub(1 + n)
            .and_then(|i| self.entries.get(i))
            .map(|e| e.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_assigns_increasing_ids() {
        let mut reg = Registry::new();
        let id0 = reg.register("Vehicle.Speed", 10.0);
        let id1 = reg.register("Vehicle.RPM", 3000.0);
        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
    }

    #[test]
    fn update_changes_the_stored_value() {
        let mut reg = Registry::new();
        let id = reg.register("Vehicle.Speed", 10.0);
        assert!(reg.update(id, 42.0));
        assert_eq!(reg.get(id), Some(42.0));
    }

    #[test]
    fn update_missing_id_returns_false() {
        let mut reg = Registry::new();
        reg.register("Vehicle.Speed", 10.0);
        assert!(!reg.update(999, 1.0));
    }

    #[test]
    fn nth_most_recent_counts_back_from_the_end() {
        let mut reg = Registry::new();
        reg.register("A", 1.0);
        reg.register("B", 2.0);
        reg.register("C", 3.0);
        assert_eq!(reg.nth_most_recent(0), Some(3.0));
        assert_eq!(reg.nth_most_recent(1), Some(2.0));
        assert_eq!(reg.nth_most_recent(2), Some(1.0));
    }
}
