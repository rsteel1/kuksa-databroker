// c10_e03_iterator_adaptors.rs
//
// CONCEPT: iterator adaptors — `iter`, `map`, `filter`, `filter_map`, and
//          `collect` into different collections.
// GUIDE:   chapters/10-closures-and-iterators/guide.md  (section "Iterator adaptors and collect")
// DATABROKER: databroker/src/broker.rs — `get_values_broker` builds a
//          `HashSet<SignalId>` with `.iter().copied().collect()` and filters
//          providers with `.filter_map(...)`.
// EXPECT:  tests fail: `todo!()` panics.
//
// Iterators are lazy: `map`/`filter` build a *recipe*; nothing runs until a
// consumer like `collect`, `count` or `for` pulls items through. `collect`
// is generic over the output — the type you ask for decides what gets built.
//
// YOUR TASK
//   Implement each function with iterator adaptors — no `for` loops, no
//   manual `push`/`insert`. Do not edit the tests.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq)]
enum EntryType {
    Sensor,
    Actuator,
}

#[derive(Debug, Clone)]
struct Metadata {
    id: i32,
    path: String,
    entry_type: EntryType,
}

/// path -> id for every entry.
fn ids_by_path(entries: &[Metadata]) -> HashMap<String, i32> {
    // WHY: collecting an iterator of (K, V) tuples builds a map.
    entries.iter().map(|m| (m.path.clone(), m.id)).collect()
}

/// Paths of actuators only, in input order. Borrow the paths, don't clone.
fn actuator_paths(entries: &[Metadata]) -> Vec<&str> {
    entries
        .iter()
        .filter(|m| m.entry_type == EntryType::Actuator)
        .map(|m| m.path.as_str())
        .collect()
}

/// Parses each string as an id, silently skipping ones that aren't numbers.
fn parse_ids(raw: &[&str]) -> Vec<i32> {
    // WHY: filter_map = map to Option + keep only the Somes.
    raw.iter().filter_map(|s| s.parse().ok()).collect()
}

/// The set of distinct top-level branches ("Vehicle" in "Vehicle.Speed").
fn root_branches(entries: &[Metadata]) -> HashSet<&str> {
    entries
        .iter()
        .filter_map(|m| m.path.split('.').next())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Metadata> {
        vec![
            Metadata {
                id: 1,
                path: "Vehicle.Speed".into(),
                entry_type: EntryType::Sensor,
            },
            Metadata {
                id: 2,
                path: "Vehicle.Door.IsOpen".into(),
                entry_type: EntryType::Actuator,
            },
            Metadata {
                id: 3,
                path: "Private.Test".into(),
                entry_type: EntryType::Sensor,
            },
            Metadata {
                id: 4,
                path: "Vehicle.Horn".into(),
                entry_type: EntryType::Actuator,
            },
        ]
    }

    #[test]
    fn builds_path_index() {
        let index = ids_by_path(&sample());
        assert_eq!(index.len(), 4);
        assert_eq!(index["Vehicle.Horn"], 4);
    }

    #[test]
    fn filters_actuators_in_order() {
        let entries = sample();
        assert_eq!(
            actuator_paths(&entries),
            vec!["Vehicle.Door.IsOpen", "Vehicle.Horn"]
        );
    }

    #[test]
    fn skips_garbage_ids() {
        assert_eq!(parse_ids(&["1", "x", "42", "", "-3"]), vec![1, 42, -3]);
    }

    #[test]
    fn distinct_roots() {
        let entries = sample();
        let roots = root_branches(&entries);
        assert_eq!(roots, HashSet::from(["Vehicle", "Private"]));
    }
}
