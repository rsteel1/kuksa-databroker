// c05_e02_hashmap_path_to_id.rs
//
// CONCEPT: HashMap<K, V> — insert, get, contains_key, and the `entry` API.
// GUIDE:   chapters/05-collections/guide.md  (section "HashMap and the entry API")
// DATABROKER: databroker/src/broker.rs — `struct Database { path_to_id, entries }`
//             and `DatabaseWriteAccess::add`, which returns the existing id
//             when a path is registered twice.
// EXPECT:  tests fail: `todo!()` panics
//
// The databroker never looks signals up by string on the hot path. It keeps
// two maps: `path_to_id: HashMap<String, i32>` (name -> id) and
// `entries: HashMap<i32, Entry>` (id -> data). You'll build the same idea.
//
// YOUR TASK
//   Implement the `todo!()`s. `register` must be idempotent: registering a
//   path that already exists returns the existing id and does not consume
//   a new one. Do not edit the tests.

// I AM NOT DONE

use std::collections::HashMap;

#[derive(Default)]
struct Registry {
    next_id: i32,
    path_to_id: HashMap<String, i32>,
    id_to_path: HashMap<i32, String>,
}

impl Registry {
    /// Returns the id for `path`, allocating a fresh one if it's new.
    fn register(&mut self, path: &str) -> i32 {
        todo!()
    }

    fn id_of(&self, path: &str) -> Option<i32> {
        todo!()
    }

    fn path_of(&self, id: i32) -> Option<&str> {
        todo!()
    }
}

/// Counts registered paths per top-level branch, e.g. "Vehicle.Speed" and
/// "Vehicle.Cabin.Door" both count towards "Vehicle".
/// Use the `entry` API: `counts.entry(key).or_insert(0)`.
fn count_by_root(paths: &[&str]) -> HashMap<String, usize> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_allocates_increasing_ids() {
        let mut r = Registry::default();
        assert_eq!(r.register("Vehicle.Speed"), 0);
        assert_eq!(r.register("Vehicle.Cabin.Door.Row1.Left.IsOpen"), 1);
    }

    #[test]
    fn register_is_idempotent() {
        let mut r = Registry::default();
        let a = r.register("Vehicle.Speed");
        let b = r.register("Vehicle.Speed");
        assert_eq!(a, b);
        assert_eq!(r.register("Vehicle.Width"), 1, "no id was wasted");
    }

    #[test]
    fn lookups_both_ways() {
        let mut r = Registry::default();
        let id = r.register("Vehicle.Speed");
        assert_eq!(r.id_of("Vehicle.Speed"), Some(id));
        assert_eq!(r.id_of("Vehicle.Nope"), None);
        assert_eq!(r.path_of(id), Some("Vehicle.Speed"));
        assert_eq!(r.path_of(99), None);
    }

    #[test]
    fn counts_per_root() {
        let counts = count_by_root(&[
            "Vehicle.Speed",
            "Vehicle.Cabin.Door",
            "Kuksa.Databroker.Version",
            "Vehicle.Width",
        ]);
        assert_eq!(counts.get("Vehicle"), Some(&3));
        assert_eq!(counts.get("Kuksa"), Some(&1));
        assert_eq!(counts.len(), 2);
    }
}
