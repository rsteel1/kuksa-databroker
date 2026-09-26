// c10_e02_fn_fnmut_fnonce.rs
//
// CONCEPT: the three closure traits `Fn`, `FnMut`, `FnOnce`, and choosing
//          the least restrictive bound for a parameter.
// GUIDE:   chapters/10-closures-and-iterators/guide.md  (section "Fn, FnMut and FnOnce")
// DATABROKER: databroker/src/broker.rs — `AuthorizedAccess::with_read_lock`
//          takes `f: impl FnOnce(&DatabaseReadAccess) -> T`, and
//          `for_each_entry` takes `f: impl FnMut(EntryReadAccess)`.
// EXPECT:  compile errors E0525 ("expected a closure that implements the `Fn`
//          trait, but this closure only implements `FnOnce`/`FnMut`"),
//          then `todo!()` panics.
//
// Every closure implements one or more of:
//   FnOnce  — can be called at least once (may consume what it captured)
//   FnMut   — can be called many times, may mutate its captures
//   Fn      — can be called many times, only reads its captures
// Fn ⊂ FnMut ⊂ FnOnce: a `Fn` closure can be passed where `FnOnce` is asked.
// So a function that calls `f` exactly once should ask for `FnOnce` — the
// weakest requirement, accepting the most closures.
//
// YOUR TASK
//   1. Change the closure bounds on `with_read` and `for_each_entry` to the
//      least restrictive trait that still works for how the function calls `f`.
//   2. Implement the three `todo!()` bodies.
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::BTreeMap;

struct Database {
    entries: BTreeMap<String, i32>,
}

impl Database {
    /// Calls `f` exactly once with read access to the entries and returns
    /// whatever `f` returns.
    fn with_read<T>(&self, f: impl Fn(&BTreeMap<String, i32>) -> T) -> T {
        todo!()
    }

    /// Calls `f` once per entry, in key order.
    fn for_each_entry(&self, f: impl Fn(&str, i32)) {
        todo!()
    }

    /// Replaces every value `v` with `f(v)`.
    fn apply_to_all(&mut self, f: impl Fn(i32) -> i32) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Database {
        let mut entries = BTreeMap::new();
        entries.insert("Vehicle.Speed".to_string(), 50);
        entries.insert("Vehicle.Cabin.Temp".to_string(), 21);
        Database { entries }
    }

    #[test]
    fn with_read_accepts_a_consuming_closure() {
        let label = String::from("entries: ");
        // This closure moves `label` out of itself: it is only FnOnce.
        let text = db().with_read(move |entries| {
            let mut label = label;
            label.push_str(&entries.len().to_string());
            label
        });
        assert_eq!(text, "entries: 2");
    }

    #[test]
    fn for_each_accepts_a_mutating_closure() {
        let mut seen = Vec::new();
        db().for_each_entry(|path, value| seen.push(format!("{path}={value}")));
        assert_eq!(seen, vec!["Vehicle.Cabin.Temp=21", "Vehicle.Speed=50"]);
    }

    #[test]
    fn apply_to_all_changes_every_value() {
        let mut db = db();
        let offset = 10;
        db.apply_to_all(|v| v + offset);
        assert_eq!(db.entries["Vehicle.Speed"], 60);
        assert_eq!(db.entries["Vehicle.Cabin.Temp"], 31);
    }
}
