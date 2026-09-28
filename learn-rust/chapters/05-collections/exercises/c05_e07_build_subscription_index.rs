// c05_e07_build_subscription_index.rs
//
// CONCEPT: writing several small functions from a spec: `HashMap`'s entry
//          API, `HashSet` union, and a `BTreeMap` for sorted output.
// GUIDE:   chapters/05-collections/guide.md  (section "HashMap and the entry API")
// DATABROKER: databroker/src/broker.rs — `Subscriptions` keeps, per
//             change subscription, the set of signal ids it cares about;
//             matching a published update against every subscription is
//             the same "which subscribers care about this path" problem.
// EXPECT:  tests fail: todo!() panics.
//
// This exercise has no missing piece for you to fix — the tests below ARE
// the spec. Implement each function so every test passes.
//
// YOUR TASK
//   Implement `subscribe`, `unsubscribe`, `matching_subscribers` and
//   `path_load` below.
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::{BTreeMap, HashMap, HashSet};

/// Adds `id` as a subscriber of `path`. Subscribing the same id twice
/// has no extra effect.
fn subscribe(index: &mut HashMap<String, HashSet<u32>>, path: &str, id: u32) {
    todo!()
}

/// Removes `id` as a subscriber of `path`. If that leaves `path` with no
/// subscribers at all, removes the path entry too (nothing should be
/// left pointing at an empty set).
fn unsubscribe(index: &mut HashMap<String, HashSet<u32>>, path: &str, id: u32) {
    todo!()
}

/// The union of every subscriber id across all of `paths` (paths not in
/// `index` contribute nothing).
fn matching_subscribers(index: &HashMap<String, HashSet<u32>>, paths: &[String]) -> HashSet<u32> {
    todo!()
}

/// How many subscribers each path currently has, sorted by path.
fn path_load(index: &HashMap<String, HashSet<u32>>) -> BTreeMap<String, usize> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribe_adds_and_dedups() {
        let mut index = HashMap::new();
        subscribe(&mut index, "Vehicle.Speed", 1);
        subscribe(&mut index, "Vehicle.Speed", 1);
        subscribe(&mut index, "Vehicle.Speed", 2);
        assert_eq!(index.get("Vehicle.Speed").unwrap().len(), 2);
    }

    #[test]
    fn unsubscribe_removes_empty_paths() {
        let mut index = HashMap::new();
        subscribe(&mut index, "Vehicle.Speed", 1);
        unsubscribe(&mut index, "Vehicle.Speed", 1);
        assert!(!index.contains_key("Vehicle.Speed"));
    }

    #[test]
    fn unsubscribe_keeps_path_while_others_remain() {
        let mut index = HashMap::new();
        subscribe(&mut index, "Vehicle.Speed", 1);
        subscribe(&mut index, "Vehicle.Speed", 2);
        unsubscribe(&mut index, "Vehicle.Speed", 1);
        assert_eq!(index.get("Vehicle.Speed").unwrap(), &HashSet::from([2]));
    }

    #[test]
    fn matching_subscribers_unions_across_paths() {
        let mut index = HashMap::new();
        subscribe(&mut index, "Vehicle.Speed", 1);
        subscribe(&mut index, "Vehicle.RPM", 2);
        subscribe(&mut index, "Vehicle.RPM", 1);
        let paths = vec!["Vehicle.Speed".to_string(), "Vehicle.RPM".to_string()];
        assert_eq!(matching_subscribers(&index, &paths), HashSet::from([1, 2]));
    }

    #[test]
    fn path_load_counts_are_sorted_by_path() {
        let mut index = HashMap::new();
        subscribe(&mut index, "Vehicle.Speed", 1);
        subscribe(&mut index, "Vehicle.Speed", 2);
        subscribe(&mut index, "Vehicle.RPM", 3);
        let load = path_load(&index);
        let entries: Vec<(String, usize)> = load.into_iter().collect();
        assert_eq!(
            entries,
            vec![
                ("Vehicle.RPM".to_string(), 1),
                ("Vehicle.Speed".to_string(), 2)
            ]
        );
    }
}
